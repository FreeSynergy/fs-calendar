//! The menus do what they say, seen through the workbench's state through the real server and client.
//!
//! Pattern: Command tests, with `AppWidget::choose`, which is what a press on a line calls. No window is
//! opened.

mod common;

use common::{World, d};
use fs_calendar_app::{labels, screen_widget};
use fs_components_workbenches::calendar::{CalendarWidget, ViewRange};

fn opened(w: &World) -> (CalendarWidget, fs_components_app::AppWidget) {
    let widget = CalendarWidget::new(w.adapter.clone(), labels());
    let app = screen_widget(widget.clone()).unwrap();
    (widget, app)
}

#[test]
fn the_view_entries_change_the_view() {
    let w = World::berlin();
    let (widget, app) = opened(&w);
    for (id, view) in [
        ("week", ViewRange::Week),
        ("day", ViewRange::Day),
        ("agenda", ViewRange::Agenda),
        ("month", ViewRange::Month),
    ] {
        assert!(app.choose(id), "{id}");
        assert_eq!(widget.with_state(|s| s.view()), view, "{id}");
    }
}

#[test]
fn the_go_entries_move_the_calendar_and_new_starts_a_form() {
    let w = World::berlin();
    let (widget, app) = opened(&w);
    assert!(app.choose("next"));
    assert_eq!(widget.with_state(|s| s.anchor()), d("2026-12-06"));
    assert!(app.choose("previous"));
    assert!(app.choose("previous"));
    assert_eq!(widget.with_state(|s| s.anchor()), d("2026-10-06"));
    assert!(app.choose("today"));
    assert_eq!(widget.with_state(|s| s.anchor()), d("2026-11-06"));
    assert!(app.choose("new"));
    let start = widget.with_state(|s| {
        s.editing()
            .unwrap()
            .draft
            .value("start")
            .map(str::to_string)
    });
    assert_eq!(start.as_deref(), Some("2026-11-06 09:00"));
}

#[test]
fn an_entry_nobody_has_does_nothing() {
    let w = World::berlin();
    let (widget, app) = opened(&w);
    assert!(!app.choose("nope"));
    assert_eq!(widget.with_state(|s| s.view()), ViewRange::Month);
}
