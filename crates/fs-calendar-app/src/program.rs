//! The window as a command: what `fs-calendar` and `fs-calendar app` run (ADR 0063).
//!
//! Pattern: Facade consumer — parse arguments, open the store and the core in this process, put the Event description
//! and its backend into the admin frontend, the calendar into the grid, and show the window (or take a picture).
//! The runtime lives as long as the window: the backend and the grid call the core on it.

use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use chrono::{Datelike, Timelike, Utc};
use fs_admin_frontend::view::{AboutInfo, AdminSurface};
use fs_admin_frontend::{Session, SystemClock};
use fs_calendar_core::{Calendar, CalendarRepo, Event, Zone};
use fs_calendar_tasks::LocalTasks;
use fs_components_app::{WindowOptions, launch};
use fs_components_calendar_grid::{CalendarGrid, Day, GridState, Mode, Moment};
use fs_components_fields::{Look, Say};
use fs_components_styles::StyleId;
use fs_objects::{Store, TursoStore};
use fs_tasks_core::TaskRepo;
use fs_types::DescribesType;

use crate::args::{Args, HELP, parse};
use crate::grid_source::CalendarSource;
use crate::grid_surface::{GridSurface, events};
use crate::window::{CALENDAR, EVENT, calendar_window};
use crate::words::Words;
use crate::{EventBackend, frontend_types, shown_zone};

/// Who acts on this store. Provisional: there is no sign-in yet.
const ACTOR: &str = "https://localhost/actors/owner";

/// Open the window (or take a picture) with `argv`, the words after `app`; the exit code.
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

fn store_file(options: &Args) -> Result<Option<PathBuf>, String> {
    if let Some(file) = &options.store {
        return Ok(Some(file.clone()));
    }
    if options.snapshot.is_some() {
        return Ok(None); // a picture alone keeps nothing
    }
    let dir = fs_interfaces::rpc::default_data_dir("calendar")
        .ok_or("no data folder: give --store or set $XDG_DATA_HOME or $HOME")?;
    let dir = fs_interfaces::rpc::prepare_data_dir(&dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?;
    Ok(Some(dir.join("objects.db")))
}

fn open(
    runtime: &tokio::runtime::Runtime,
    options: &Args,
    zone: &Zone,
) -> Result<Arc<dyn Calendar>, String> {
    let file = store_file(options)?;
    let store: Arc<dyn Store> = runtime
        .block_on(async {
            match &file {
                Some(file) => TursoStore::open(file).await,
                None => TursoStore::open_in_memory().await,
            }
        })
        .map(|s| Arc::new(s) as Arc<dyn Store>)
        .map_err(|e| format!("cannot open the store: {e}"))?;
    let tasks = Arc::new(TaskRepo::new(store.clone(), ACTOR));
    // The task store is in this process; the core of the calendar sees it only through its two traits.
    let local = Arc::new(LocalTasks::new(tasks));
    let repo = CalendarRepo::new(store, ACTOR)
        .with_floating_zone(zone.clone())
        .with_tasks(local.clone())
        .with_task_intake(local);
    Ok(Arc::new(repo))
}

fn today(zone: &Zone) -> (Day, Moment) {
    let now = zone.to_local(Utc::now());
    let day = Day::from_ymd(
        now.year(),
        u8::try_from(now.month()).unwrap_or(1),
        u8::try_from(now.day()).unwrap_or(1),
    );
    (
        day,
        Moment::new(
            day,
            u16::try_from(now.hour() * 60 + now.minute()).unwrap_or(0),
        ),
    )
}

fn run(argv: &[String]) -> Result<(), String> {
    let options = parse(argv)?;
    if options.help {
        println!("{HELP}");
        return Ok(());
    }
    let (zone, zone_note) = shown_zone(options.zone.as_deref())?;
    if let Some(note) = zone_note {
        eprintln!("{note}");
    }
    let runtime = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
    let calendar = open(&runtime, &options, &zone)?;
    let handle = runtime.handle().clone();

    let say: Arc<dyn Say> = Arc::new(Words);
    let look = Look::new(StyleId::Admin, false);
    let backend = Arc::new(EventBackend::new(calendar.clone(), handle.clone()));
    let description = frontend_types::description(&Event::describe());
    let session = Session::new(description, backend, Arc::new(SystemClock::new()));
    let admin = Arc::new(AdminSurface::new(session, look, say.clone(), None));

    let (day, now) = today(&zone);
    let mut state = GridState::new(Mode::Month, day);
    state.today = Some(day);
    state.now = Some(now);
    let source = Arc::new(CalendarSource::new(calendar, handle, zone));
    let shell: Arc<OnceLock<fs_components_app::AppWidget>> = Arc::new(OnceLock::new());
    let (open_shell, open_admin) = (shell.clone(), admin.clone());
    let grid = CalendarGrid::new(state, source, say.clone(), look).events(events(move |key| {
        // an occurrence names its series; a task is not ours to edit
        let id = key.split('#').next().unwrap_or(key);
        if key.starts_with("task:") {
            return;
        }
        {
            let mut s = open_admin
                .session()
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            s.open_list();
            s.open_detail(id);
        }
        if let Some(app) = open_shell.get() {
            app.select(EVENT);
        }
    }));
    let about = AboutInfo {
        name: "FreeSynergy.Calendar".into(),
        version: format!("v{}", env!("CARGO_PKG_VERSION")),
        origin: "source".into(),
        licence: env!("CARGO_PKG_LICENSE").into(),
        repository: env!("CARGO_PKG_REPOSITORY").into(),
    };
    let app = calendar_window(
        "FreeSynergy.Calendar",
        &say,
        GridSurface(grid),
        admin,
        about,
        env!("CARGO_PKG_REPOSITORY").into(),
        &shell,
    )
    .map_err(|e| e.to_string())?;
    debug_assert!(app.with_shell(|s| s.current().is_some_and(|c| c.id == CALENDAR)));
    let (width, height) = options.size;
    if let Some(file) = &options.snapshot {
        app.picture(width, height, file)
            .map_err(|e| e.to_string())?;
        eprintln!("wrote {}", file.display());
        return Ok(());
    }
    let mut window = WindowOptions::default();
    window.width = width;
    window.height = height;
    launch(window, app).map_err(|e| e.to_string())
}
