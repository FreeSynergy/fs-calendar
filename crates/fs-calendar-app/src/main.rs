//! The binary: wiring only.
//!
//! Pattern: Facade consumer — parse arguments, open the store, import, then export or show. The runtime
//! lives as long as the window: the workbench's calls run on it.

use std::sync::Arc;

use fs_calendar_app::args::{Args, HELP, parse};
use fs_calendar_app::{
    CalendarAdapter, export_all, import_file, labels, screen_widget, shown_zone,
};
use fs_calendar_core::{Calendar, CalendarRepo};
use fs_components_app::{Status, WindowOptions, launch};
use fs_components_workbenches::calendar::{CalendarWidget, Moment};
use fs_objects::{Store, TursoStore};
use fs_tasks_core::{TaskItems, TaskRepo};

/// Who acts on this store. Provisional: there is no sign-in yet.
const ACTOR: &str = "https://localhost/actors/owner";

fn main() {
    if let Err(message) = run() {
        eprintln!("fs-calendar-app: {message}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let args = parse(&argv)?;
    if args.help {
        println!("{HELP}");
        return Ok(());
    }
    let (zone, zone_note) = shown_zone(args.zone.as_deref())?;
    let runtime = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
    let store: Arc<dyn Store> = runtime
        .block_on(async {
            match &args.store {
                Some(file) => TursoStore::open(file).await,
                None => TursoStore::open_in_memory().await,
            }
        })
        .map(|s| Arc::new(s) as Arc<dyn Store>)
        .map_err(|e| format!("cannot open the store: {e}"))?;
    let tasks: Arc<dyn TaskItems> = Arc::new(TaskRepo::new(store.clone(), ACTOR));
    let repo: Arc<dyn Calendar> = Arc::new(
        CalendarRepo::new(store, ACTOR)
            .with_floating_zone(zone.clone())
            .with_tasks(tasks.clone()),
    );
    let handle = runtime.handle().clone();

    let mut status = None;
    if let Some(file) = &args.import {
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
    if let Some(file) = &args.export {
        let report = export_all(repo.as_ref(), &handle, file, &args.export_sensitive)?;
        eprintln!("{}", report.message(file));
        if !report.failed.is_empty() {
            return Err(format!("{} could not be exported", report.failed.len()));
        }
        if args.snapshot.is_none() {
            return Ok(());
        }
    }
    show(&args, repo, tasks, handle, zone, status)
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
