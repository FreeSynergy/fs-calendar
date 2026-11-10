//! The frontend description and the window: what the old frontend is given, and the standard menus of ADR 0065.
//!
//! Pattern: none (tests).
#![allow(clippy::pedantic)]

use std::sync::{Arc, OnceLock};

use fs_admin_frontend::view::{AboutInfo, AdminSurface};
use fs_admin_frontend::{Session, SystemClock};
use fs_calendar_app::frontend_types::description;
use fs_calendar_app::{CALENDAR, CalendarSource, EVENT, EventBackend, Words, calendar_window};
use fs_calendar_core::{Calendar, CalendarRepo, Event, Zone};
use fs_components_calendar_grid::{CalendarGrid, Day, GridState, Mode};
use fs_components_fields::{Look, Say};
use fs_components_styles::StyleId;
use fs_objects::{Store, TursoStore};
use fs_types::DescribesType;

#[test]
fn the_old_frontend_gets_a_sound_description_with_a_change_form() {
    let new = Event::describe();
    let old = description(&new);
    let mut registry = fs_types_frontend::TypeRegistry::new();
    registry
        .add(old.clone())
        .expect("the old registry accepts it");
    assert_eq!(old.commands.len(), new.commands.len());
    assert_eq!(old.fields.len(), new.fields.len());
    let change = old.find_command("change").unwrap();
    assert_eq!(change.input[0].name, "id");
    assert_eq!(change.input[0].target, "Event");
    assert_eq!(change.input.len(), new.fields.len() + 1);
    assert!(change.input[1..].iter().all(|f| !f.required));
    // every command, supported or not, is there for the frontend (the guard in `run` refuses the others)
    for c in &new.commands {
        assert!(old.find_command(&c.name).is_some(), "{}", c.name);
    }
}

#[test]
fn the_window_has_the_standard_menus_and_both_screens() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let store: Arc<dyn Store> = Arc::new(rt.block_on(TursoStore::open_in_memory()).unwrap());
    let calendar: Arc<dyn Calendar> = Arc::new(CalendarRepo::new(store, "https://local.invalid/u"));
    let say: Arc<dyn Say> = Arc::new(Words);
    let look = Look::new(StyleId::Admin, false);
    let backend = Arc::new(EventBackend::new(calendar.clone(), rt.handle().clone()));
    let session = Session::new(
        description(&Event::describe()),
        backend,
        Arc::new(SystemClock::new()),
    );
    let admin = Arc::new(AdminSurface::new(session, look.clone(), say.clone(), None));
    let state = GridState::new(Mode::Month, Day::from_ymd(2026, 11, 10));
    let source = Arc::new(CalendarSource::new(
        calendar,
        rt.handle().clone(),
        Zone::Utc,
    ));
    let grid = CalendarGrid::new(state, source, say.clone(), look);
    let about = AboutInfo {
        name: "FreeSynergy.Calendar".into(),
        version: format!("v{}", env!("CARGO_PKG_VERSION")),
        origin: "source".into(),
        licence: "MIT OR Apache-2.0".into(),
        repository: "https://github.com/FreeSynergy/fs-calendar".into(),
    };
    let shell = Arc::new(OnceLock::new());
    let app = calendar_window(
        "FreeSynergy.Calendar",
        &say,
        fs_calendar_app::GridSurface(grid),
        admin,
        about,
        "https://example.org".into(),
        &shell,
    )
    .unwrap();
    app.with_shell(|s| {
        let ids: Vec<&str> = s
            .menus()
            .iter()
            .flat_map(|m| m.entries.iter().map(|e| e.id.as_str()))
            .collect();
        assert_eq!(
            ids,
            [
                "program-quit",
                "view-Calendar",
                "view-Event",
                "help-help",
                "help-about"
            ]
        );
        assert_eq!(s.current().map(|c| c.id.as_str()), Some(CALENDAR));
    });
    app.select(EVENT);
    assert_eq!(
        app.with_shell(|s| s.current().map(|c| c.id.clone()))
            .as_deref(),
        Some(EVENT)
    );
}
