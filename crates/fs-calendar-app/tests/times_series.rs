//! Clock changes, series across zones, week start and the far future.
//!
//! Pattern: Table tests over the real core.

mod common;

use std::sync::Arc;

use common::{World, d};
use fs_calendar_app::CalendarAdapter;
use fs_calendar_core::{Event, Zone};
use fs_components_workbenches::calendar::{CalendarSource, Moment, Shown, ViewRange};
use serde_json::json;

fn event(title: &str, start: &str, zone: Option<&str>, duration: &str) -> Event {
    let mut e = Event::new(title, start, zone.unwrap_or("Etc/UTC"), duration);
    e.time_zone = zone.map(str::to_string);
    e
}

fn m(day: &str, hour: u16, minute: u16) -> Moment {
    Moment::at(d(day), hour, minute)
}

#[test]
fn a_day_when_the_clocks_change_keeps_clock_times_and_real_lengths() {
    let w = World::berlin();
    // 2026-10-25: at 03:00 summer time clocks go back to 02:00. 22:00 to 04:00 is seven real hours.
    let key = w.add(event(
        "Autumn",
        "2026-10-24T22:00:00",
        Some("Europe/Berlin"),
        "PT7H",
    ));
    let item = w.item("2026-10-25", "Autumn");
    assert_eq!(item.end, m("2026-10-25", 4, 0));
    let draft = w.adapter.draft(&key).unwrap();
    assert_eq!(draft.value("end"), Some("2026-10-25 04:00"));
    let mut s = w.state();
    s.select(&key);
    s.begin_edit();
    s.set_field("start", "2026-10-24 23:00");
    s.save();
    assert!(s.editing().is_none(), "{:?}", s.notice());
    assert_eq!(
        w.stored(&key).content["duration"],
        json!("PT6H"),
        "real hours to the same clock end"
    );
    assert_eq!(w.item("2026-10-25", "Autumn").end, m("2026-10-25", 4, 0));

    // 2027-03-28: at 02:00 clocks jump to 03:00. A nominal day is 23 real hours and ends on the clock.
    let spring = w.add(event(
        "Spring",
        "2027-03-27T12:00:00",
        Some("Europe/Berlin"),
        "P1D",
    ));
    assert_eq!(w.item("2027-03-28", "Spring").end, m("2027-03-28", 12, 0));
    assert_eq!(
        w.adapter.draft(&spring).unwrap().value("end"),
        Some("2027-03-28 12:00")
    );

    // A daily event keeps its clock time across the change.
    let mut daily = event(
        "Daily",
        "2026-10-24T09:00:00",
        Some("Europe/Berlin"),
        "PT1H",
    );
    daily.recurrence_rule =
        serde_json::from_value(json!({"frequency": "daily", "count": 3})).unwrap();
    w.add(daily);
    for day in ["2026-10-24", "2026-10-25", "2026-10-26"] {
        assert_eq!(w.item(day, "Daily").start, m(day, 9, 0), "{day}");
    }
}

#[test]
fn a_series_in_another_zone_moves_across_the_days_of_the_zone_shown() {
    let w = World::shown_in("Pacific/Auckland");
    let mut weekly = event(
        "Weekly",
        "2026-11-03T20:00:00",
        Some("America/New_York"),
        "PT1H",
    );
    weekly.recurrence_rule = serde_json::from_value(json!({"frequency": "weekly"})).unwrap();
    w.add(weekly);
    let item = w.item("2026-11-04", "Weekly");
    assert_eq!(
        item.start,
        m("2026-11-04", 14, 0),
        "20:00 EST is 01:00 UTC is 14:00 NZDT"
    );
}

#[test]
fn the_week_start_shows_the_right_days() {
    let w = World::berlin();
    w.add(event(
        "Sunday",
        "2026-11-01T10:00:00",
        Some("Europe/Berlin"),
        "PT1H",
    ));
    let adapter = CalendarAdapter::new(
        w.repo.clone(),
        w.runtime.handle().clone(),
        Zone::parse("Europe/Berlin").unwrap(),
    )
    .frozen_at(m("2026-11-06", 12, 0))
    .week_start(6);
    let s = fs_components_workbenches::calendar::CalendarState::new(
        Arc::new(adapter),
        fs_calendar_app::labels(),
        fs_components_workbenches::calendar::Variant::Full,
    );
    assert_eq!(s.week_start(), 6);
    let mut s = s;
    s.set_view(ViewRange::Week);
    assert_eq!(s.span().0, d("2026-11-01"));
    assert_eq!(s.items().len(), 1, "that Sunday is in this week");
    let mut monday = w.state();
    monday.set_view(ViewRange::Week);
    assert!(
        monday.items().is_empty(),
        "with Monday first it is last week's"
    );
}

#[test]
fn a_far_future_date_shows_an_endless_series_and_a_single_event() {
    let w = World::berlin();
    let mut weekly = event(
        "Endless",
        "2026-11-03T10:00:00",
        Some("Europe/Berlin"),
        "PT1H",
    );
    weekly.recurrence_rule = serde_json::from_value(json!({"frequency": "weekly"})).unwrap();
    let key = w.add(weekly);
    w.add(event(
        "One far",
        "2999-06-15T10:00:00",
        Some("Europe/Berlin"),
        "PT1H",
    ));
    let mut s = w.state();
    s.go_to(d("2999-06-10"));
    assert_eq!(s.problem(), None);
    assert!(s.items().iter().any(|i| i.title == "One far"));
    let far: Vec<_> = s.items().iter().filter(|i| i.title == "Endless").collect();
    assert!(far.len() >= 4, "{far:?}");
    let first = far[0].key.clone();
    assert!(first.starts_with(&key));
    s.select(&first);
    assert!(matches!(s.shown(), Shown::Item { .. }), "{:?}", s.shown());
    s.go_to(d("2026-11-06"));
    assert!(s.items().iter().any(|i| i.title == "Endless"));
}

#[test]
fn the_far_end_of_the_calendar_answers_for_an_endless_daily_series() {
    let w = World::berlin();
    let mut daily = event(
        "Daily",
        "2020-01-01T10:00:00",
        Some("Europe/Berlin"),
        "PT1H",
    );
    daily.recurrence_rule = serde_json::from_value(json!({"frequency": "daily"})).unwrap();
    w.add(daily);
    let started = std::time::Instant::now();
    // The core walks a rule from its start and refuses a window beyond `MAX_RULE_STEPS` of it (about
    // the year 3389 for a daily series from 2020), so the far end asked for is a year within it.
    let items = w.items("3300-12-01", "3300-12-07");
    eprintln!(
        "7 days of year 3300: {} items in {:?}",
        items.len(),
        started.elapsed()
    );
    let started = std::time::Instant::now();
    let near = w.items("2026-12-01", "2026-12-07");
    eprintln!(
        "7 days of year 2026: {} items in {:?}",
        near.len(),
        started.elapsed()
    );
    assert_eq!(near.len(), 7);
    assert_eq!(items.len(), 7, "the series does not stop in the far future");
}
