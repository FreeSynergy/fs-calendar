//! The whole program as a picture, through the shell's real path. No window is opened.
//!
//! Pattern: Pixel tests — skipped with a message where there is no graphics adapter. Pictures are kept
//! where `FS_COMP_SHOTS` says. A hang here is a deadlock.

mod common;

use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use common::{World, d, rich, rich_series};
use fs_calendar_app::{labels, screen_widget};
use fs_calendar_core::Event;
use fs_components_app::{AppError, Status};
use fs_components_render::{Canvas, RenderError, draw_to_png};
use fs_components_workbenches::calendar::{CalendarWidget, ViewRange};
use serde_json::json;

static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

fn serial() -> MutexGuard<'static, ()> {
    ONE_AT_A_TIME.lock().unwrap_or_else(PoisonError::into_inner)
}

fn out(name: &str) -> PathBuf {
    let dir = std::env::var_os("FS_COMP_SHOTS").map_or_else(
        || std::env::temp_dir().join(format!("fs-calendar-app-{}", std::process::id())),
        PathBuf::from,
    );
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

/// The picture's bytes; it stays only where `FS_COMP_SHOTS` asks for it.
fn read_and_clear(path: &PathBuf) -> Vec<u8> {
    let bytes = std::fs::read(path).unwrap();
    if std::env::var_os("FS_COMP_SHOTS").is_none() {
        std::fs::remove_file(path).unwrap();
        let _ = std::fs::remove_dir(path.parent().unwrap());
    }
    bytes
}

fn event(title: &str, start: &str, duration: &str, i: u32) -> Event {
    let mut e = Event::new(title, start, "Europe/Berlin", duration);
    e.uid = format!("urn:uuid:01a1aaf0-08a0-756c-85b2-f092789c{i:04x}");
    e
}

fn world() -> World {
    let w = World::berlin();
    w.add_json(rich());
    let mut series = rich_series();
    series["recurrenceRule"] = json!({"frequency": "weekly"});
    w.add_json(series);
    w.add(event("Standup", "2026-11-04T09:00:00", "PT30M", 1));
    w.add(event("Review", "2026-11-04T09:15:00", "PT1H45M", 2));
    w.add(event(
        "Budget planning with the very long title that cannot fit",
        "2026-11-04T09:20:00",
        "PT1H",
        3,
    ));
    w.add(event("Night shift", "2026-11-05T22:00:00", "PT6H", 4));
    let mut holiday = event("Holiday", "2026-11-09T00:00:00", "P2D", 5);
    holiday.show_without_time = Some(true);
    holiday.time_zone = None;
    w.add(holiday);
    let mut clinic = event("Clinic", "2026-11-06T08:00:00", "PT1H", 6);
    clinic.set_vendor("freesynergy.net:sensitive", json!(["pd:HealthRecord"]));
    w.add(clinic);
    w.tasks.add("Send the minutes", Some("2026-11-06T17:00:00"));
    for i in 0..6 {
        w.add(event(
            &format!("Busy {i}"),
            &format!("2026-11-18T{:02}:00:00", 8 + i),
            "PT30M",
            10 + i,
        ));
    }
    w
}

#[test]
fn the_program_draws_through_picture_and_through_the_live_build_alike() {
    let _one = serial();
    let w = world();
    let app = fs_calendar_app::app_widget(w.adapter.clone()).unwrap();
    app.set_status(Some(Status::Info("Imported 3, 0 already known".into())));
    let once = out("calendar-app.png");
    match app.picture(1300, 800, &once) {
        Err(AppError::Picture(RenderError::NoGraphics(why))) => {
            eprintln!("skipped: {why}");
            return;
        }
        other => other.unwrap(),
    }
    let live = out("calendar-app-live.png");
    let canvas = Canvas::new(1300, 800, app.styles());
    draw_to_png(
        &canvas,
        |width, height| app.build(width, height),
        || {},
        &live,
    )
    .unwrap();
    let (a, b) = (read_and_clear(&once), read_and_clear(&live));
    assert!(!a.is_empty() && !b.is_empty());
    assert_eq!(a, b, "the window's path draws what the picture draws");
}

#[test]
fn the_views_the_form_and_a_marked_event_are_drawn() {
    let _one = serial();
    let w = world();
    let widget = CalendarWidget::new(w.adapter.clone(), labels());
    let app = screen_widget(widget.clone()).unwrap();
    let draw = |name: &str| -> Option<Vec<u8>> {
        let path = out(name);
        match app.picture(1300, 800, &path) {
            Err(AppError::Picture(RenderError::NoGraphics(_))) => None,
            other => {
                other.unwrap();
                Some(read_and_clear(&path))
            }
        }
    };
    let Some(month) = draw("calendar-month.png") else {
        return;
    };
    widget.with_state(|s| s.set_view(ViewRange::Week));
    let week = draw("calendar-week.png").unwrap();
    assert_ne!(month, week);
    widget.with_state(|s| s.set_view(ViewRange::Day));
    widget.with_state(|s| s.go_to(d("2026-11-04")));
    let day = draw("calendar-day.png").unwrap();
    assert_ne!(week, day);
    widget.with_state(|s| s.set_view(ViewRange::Agenda));
    widget.with_state(|s| s.go_to(d("2026-11-04")));
    let agenda = draw("calendar-agenda.png").unwrap();
    assert_ne!(day, agenda);
    let clinic = widget.with_state(|s| {
        s.go_to(d("2026-11-06"));
        s.items()
            .iter()
            .find(|i| i.title == "Clinic")
            .unwrap()
            .key
            .clone()
    });
    widget.with_state(|s| s.select(&clinic));
    let marked = draw("calendar-marked.png").unwrap();
    assert_ne!(agenda, marked);
    let series = widget.with_state(|s| {
        s.items()
            .iter()
            .find(|i| i.title == "Jour fixe")
            .unwrap()
            .key
            .clone()
    });
    widget.with_state(|s| s.select(&series));
    let sheet = draw("calendar-series.png").unwrap();
    assert_ne!(marked, sheet);
    widget.with_state(|s| s.begin_edit());
    let form = draw("calendar-form-series.png").unwrap();
    assert_ne!(sheet, form);
    widget.with_state(|s| {
        s.cancel_edit();
        s.close();
        s.set_view(ViewRange::Week);
        s.begin_create();
    });
    let new_form = draw("calendar-form-new.png").unwrap();
    assert_ne!(form, new_form);
    let _ = Arc::new(0);
}
