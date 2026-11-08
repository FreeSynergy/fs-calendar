//! The window as a command: what `fs-calendar` and `fs-calendar app` run (ADR 0063).
//!
//! Pattern: Facade consumer — parse arguments, open the store, import, then export or show. The runtime
//! lives as long as the window: the workbench's calls run on it.

use std::sync::Arc;

use crate::args::{Args, HELP, parse};
use crate::{CalendarAdapter, export_all, import_file, labels, screen_widget, shown_zone};
use fs_calendar_core::{Calendar, CalendarRepo};
use fs_calendar_tasks::LocalTasks;
use fs_components_app::{Status, WindowOptions, launch};
use fs_components_workbenches::calendar::{CalendarWidget, Moment};
use fs_objects::{Store, TursoStore};
use fs_tasks_core::{TaskItems, TaskRepo};

/// Who acts on this store. Provisional: there is no sign-in yet.
const ACTOR: &str = "https://localhost/actors/owner";

/// Open the window (or import, export, take a picture) with `argv`, the words after `app`; the exit code.
#[must_use]
pub fn app_command(argv: &[String]) -> i32 {
    match run(argv) {
        Ok(()) => 0,
        Err(message) => {
            eprintln!("fs-calendar: {message}");
            1
        }
    }
}

fn run(argv: &[String]) -> Result<(), String> {
    let options = parse(argv)?;
    if options.help {
        println!("{HELP}");
        return Ok(());
    }
    let (zone, zone_note) = shown_zone(options.zone.as_deref())?;
    let runtime = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
    // A picture alone needs no server; with `--socket` it draws what that server holds.
    let picture_only = options.snapshot.is_some() && options.socket.is_none();
    let local = options.local
        || picture_only
        || options.store.is_some()
        || options.import.is_some()
        || options.export.is_some();
    let (repo, tasks) = if local {
        open_local(&runtime, &options, &zone)?
    } else {
        open_remote(&runtime, &options)?
    };
    let handle = runtime.handle().clone();

    let mut status = None;
    if let Some(file) = &options.import {
        let summary = import_file(repo.as_ref(), &handle, file)?;
        eprintln!("{}", summary.message());
        status = Some(if summary.is_clean() {
            Status::Info(summary.message())
        } else {
            Status::Problem(summary.message())
        });
    }
    if let Some(note) = zone_note {
        eprintln!("{note}");
        status = Some(Status::Problem(match status {
            Some(Status::Info(text) | Status::Problem(text)) => format!("{note}. {text}"),
            _ => note,
        }));
    }
    if let Some(file) = &options.export {
        let report = export_all(repo.as_ref(), &handle, file, &options.export_sensitive)?;
        eprintln!("{}", report.message(file));
        if !report.failed.is_empty() {
            return Err(format!("{} could not be exported", report.failed.len()));
        }
        if options.snapshot.is_none() {
            return Ok(());
        }
    }
    show(&options, repo, tasks, handle, zone, status)
}

/// The calendar and the task source the window reads.
type Sources = (Arc<dyn Calendar>, Arc<dyn TaskItems>);

/// The events and tasks in this process (the old way; also for files).
fn open_local(
    runtime: &tokio::runtime::Runtime,
    options: &Args,
    zone: &fs_calendar_core::Zone,
) -> Result<Sources, String> {
    let store: Arc<dyn Store> = runtime
        .block_on(async {
            match &options.store {
                Some(file) => TursoStore::open(file).await,
                None => TursoStore::open_in_memory().await,
            }
        })
        .map(|s| Arc::new(s) as Arc<dyn Store>)
        .map_err(|e| format!("cannot open the store: {e}"))?;
    let tasks: Arc<dyn TaskItems> = Arc::new(TaskRepo::new(store.clone(), ACTOR));
    // The task store is in this process; the core of the calendar sees it only through its two traits.
    let local = Arc::new(LocalTasks::new(tasks.clone()));
    let repo: Arc<dyn Calendar> = Arc::new(
        CalendarRepo::new(store, ACTOR)
            .with_floating_zone(zone.clone())
            .with_tasks(local.clone())
            .with_task_intake(local),
    );
    Ok((repo, tasks))
}

/// The events at the calendar server. Tasks are not asked over the socket yet: the window shows events
/// alone, with an empty task store of its own so that nothing else changes.
fn open_remote(runtime: &tokio::runtime::Runtime, options: &Args) -> Result<Sources, String> {
    let socket = match &options.socket {
        Some(socket) => socket.clone(),
        None => fs_interfaces::rpc::default_socket("calendar")
            .ok_or("no $XDG_RUNTIME_DIR: pass --socket")?,
    };
    // A socket given with `--socket` is never answered by starting anything: the person chose it.
    let remote = connect_or_start(&socket, options.socket.is_none())?;
    let store: Arc<dyn Store> = runtime
        .block_on(TursoStore::open_in_memory())
        .map(|s| Arc::new(s) as Arc<dyn Store>)
        .map_err(|e| e.to_string())?;
    Ok((Arc::new(remote), Arc::new(TaskRepo::new(store, ACTOR))))
}

/// Connect to the server; if none answers on the user's own socket, start `fs-calendar serve` (detached,
/// so other windows can use it too and it keeps the events after this window closes) and wait for it.
fn connect_or_start(
    socket: &std::path::Path,
    may_start: bool,
) -> Result<crate::RemoteCalendar, String> {
    let first = crate::RemoteCalendar::connect(socket);
    if first.is_ok() || !may_start {
        return first;
    }
    let me = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut server = std::process::Command::new(me);
    server
        .arg("serve")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut server, 0);
    server
        .spawn()
        .map_err(|e| format!("could not start the calendar server: {e}"))?;
    for _ in 0..50 {
        std::thread::sleep(std::time::Duration::from_millis(100));
        if let Ok(remote) = crate::RemoteCalendar::connect(socket) {
            return Ok(remote);
        }
    }
    first
}

fn show(
    args: &Args,
    repo: Arc<dyn Calendar>,
    tasks: Arc<dyn TaskItems>,
    handle: tokio::runtime::Handle,
    zone: fs_calendar_core::Zone,
    status: Option<Status>,
) -> Result<(), String> {
    let mut adapter = CalendarAdapter::new(repo, handle, zone)
        .with_tasks(tasks)
        .week_start(args.week_start);
    if let Some(day) = args.date {
        adapter = adapter.frozen_at(Moment::at(day, 12, 0));
    }
    let widget = CalendarWidget::new(Arc::new(adapter), labels());
    if let Some(view) = args.view {
        widget.with_state(|s| s.set_view(view));
    }
    let app = screen_widget(widget).map_err(|e| e.to_string())?;
    app.set_status(status);
    let (width, height) = args.size;
    if let Some(file) = &args.snapshot {
        app.picture(width, height, file)
            .map_err(|e| e.to_string())?;
        eprintln!("wrote {}", file.display());
        return Ok(());
    }
    let mut options = WindowOptions::default();
    options.width = width;
    options.height = height;
    launch(options, app).map_err(|e| e.to_string())
}
