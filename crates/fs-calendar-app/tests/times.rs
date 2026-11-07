//! All-day events, time zones, floating events, events over midnight and over months.
//!
//! Table tests through the real server and client: what is stored and what the calendar view is given.

mod common;

use common::{World, d, sheet_text};
use fs_calendar_core::Event;
use fs_components_workbenches::calendar::{CalendarSource, Moment};
use serde_json::json;

fn event(title: &str, start: &str, zone: Option<&str>, duration: &str) -> Event {
    let mut e = Event::new(title, start, zone.unwrap_or("Etc/UTC"), duration);
    e.time_zone = zone.map(str::to_string);
    e
}

fn all_day(title: &str, start: &str, duration: &str) -> Event {
    let mut e = event(title, start, None, duration);
    e.show_without_time = Some(true);
    e
}

fn m(day: &str, hour: u16, minute: u16) -> Moment {
    Moment::at(d(day), hour, minute)
}

#[test]
fn an_all_day_event_stays_on_its_day_in_any_zone_shown() {
    for zone in ["Pacific/Auckland", "Pacific/Honolulu", "Europe/Berlin"] {
        let w = World::shown_in(zone);
        w.add(all_day("Holiday", "2026-11-24T00:00:00", "P1D"));
        let item = w.item("2026-11-24", "Holiday");
        assert!(item.all_day);
        assert_eq!(item.start, Moment::start_of(d("2026-11-24")), "{zone}");
        assert!(w.items("2026-11-23", "2026-11-23").is_empty(), "{zone}");
        assert!(w.items("2026-11-25", "2026-11-25").is_empty(), "{zone}");
    }
}

#[test]
fn a_many_day_all_day_event_crosses_a_month_and_the_form_says_its_last_day() {
    let w = World::berlin();
    let key = w.add(all_day("Trip", "2026-11-29T00:00:00", "P3D"));
    for day in ["2026-11-29", "2026-11-30", "2026-12-01"] {
        assert_eq!(w.items(day, day).len(), 1, "{day}");
    }
    assert!(w.items("2026-12-02", "2026-12-02").is_empty());
    let draft = w.adapter.draft(&key).unwrap();
    assert_eq!(draft.value("start"), Some("2026-11-29"));
    assert_eq!(draft.value("end"), Some("2026-12-01"));
    assert_eq!(draft.value("all_day"), Some("true"));
    assert_eq!(draft.value("time_zone"), Some(""));
}

#[test]
fn an_imported_all_day_event_is_all_day() {
    let w = World::berlin();
    let ics = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//x//y//EN\r\nBEGIN:VEVENT\r\nUID:ad-1\r\nDTSTAMP:20261101T000000Z\r\nDTSTART;VALUE=DATE:20261124\r\nDTEND;VALUE=DATE:20261126\r\nSUMMARY:Imported days\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
    w.runtime.block_on(w.repo.import_ics(ics)).unwrap();
    let item = w.item("2026-11-25", "Imported days");
    assert!(item.all_day, "{item:?}");
    assert_eq!(item.start.day, d("2026-11-24"));
    assert!(w.items("2026-11-26", "2026-11-26").is_empty());
}

#[test]
fn turning_a_timed_event_into_all_day_and_back_is_what_the_form_says() {
    let w = World::berlin();
    let key = w.add(event(
        "Flip",
        "2026-11-24T22:00:00",
        Some("Asia/Tokyo"),
        "PT3H",
    ));
    let mut s = w.state();
    s.select(&key);
    s.begin_edit();
    s.toggle_field("all_day");
    s.save();
    assert!(s.editing().is_none(), "{:?}", s.notice());
    let stored = w.stored(&key).content;
    assert_eq!(stored["showWithoutTime"], json!(true));
    assert_eq!(stored["start"], json!("2026-11-24T00:00:00"));
    assert_eq!(
        stored["duration"],
        json!("P2D"),
        "22:00 to 01:00 touches two days"
    );
    assert_eq!(stored.get("timeZone"), None);
    assert!(w.item("2026-11-25", "Flip").all_day);

    s.begin_edit();
    s.toggle_field("all_day");
    s.save();
    assert!(
        s.editing()
            .unwrap()
            .error
            .as_deref()
            .unwrap()
            .contains("with a time")
    );
    s.set_field("start", "2026-11-24 10:00");
    s.set_field("end", "2026-11-24 11:00");
    s.set_field("time_zone", "Europe/Berlin");
    s.save();
    assert!(s.editing().is_none(), "{:?}", s.notice());
    let stored = w.stored(&key).content;
    assert_eq!(stored.get("showWithoutTime"), None);
    assert_eq!(stored["start"], json!("2026-11-24T10:00:00"));
    assert_eq!(stored["duration"], json!("PT1H"));
    assert!(!w.item("2026-11-24", "Flip").all_day);
}

#[test]
fn an_event_in_another_zone_is_shown_on_the_clock_of_the_zone_shown() {
    let w = World::berlin();
    let key = w.add(event(
        "Tokyo call",
        "2026-11-25T01:00:00",
        Some("Asia/Tokyo"),
        "PT1H",
    ));
    let item = w.item("2026-11-24", "Tokyo call");
    assert_eq!(
        (item.start, item.end),
        (m("2026-11-24", 17, 0), m("2026-11-24", 18, 0))
    );
    assert!(
        w.items("2026-11-25", "2026-11-25").is_empty(),
        "the 25th only in Tokyo"
    );
    let mut s = w.state();
    s.select(&key);
    let text = sheet_text(&s);
    assert!(text.contains("Start: 2026-11-25 01:00"), "{text}");
    assert!(
        text.contains("In the zone shown: 2026-11-24 17:00"),
        "{text}"
    );
    s.begin_edit();
    assert_eq!(
        s.editing().unwrap().draft.value("start"),
        Some("2026-11-25 01:00"),
        "its own zone"
    );
}

#[test]
fn a_floating_event_keeps_its_clock_in_any_zone_shown() {
    for zone in ["Asia/Tokyo", "America/New_York"] {
        let w = World::shown_in(zone);
        w.add(event("Coffee", "2026-11-24T09:00:00", None, "PT30M"));
        let item = w.item("2026-11-24", "Coffee");
        assert_eq!(item.start, m("2026-11-24", 9, 0), "{zone}");
    }
}

#[test]
fn an_event_over_midnight_is_on_both_days_and_over_a_month_end_too() {
    let w = World::berlin();
    w.add(event(
        "Night",
        "2026-11-30T22:00:00",
        Some("Europe/Berlin"),
        "PT4H",
    ));
    let night = w.item("2026-11-30", "Night");
    assert_eq!(night.end, m("2026-12-01", 2, 0));
    assert_eq!(w.item("2026-12-01", "Night").key, night.key);
    assert!(w.items("2026-12-02", "2026-12-02").is_empty());
}

#[test]
fn a_new_event_in_a_zone_that_is_not_the_one_shown_lands_on_the_right_day() {
    let w = World::berlin();
    let mut s = w.state();
    s.begin_create();
    s.set_field("title", "Abroad");
    s.set_field("start", "2027-02-01 00:30");
    s.set_field("end", "2027-02-01 01:30");
    s.set_field("time_zone", "Asia/Tokyo");
    s.save();
    assert!(s.editing().is_none(), "{:?}", s.notice());
    assert_eq!(s.anchor(), d("2027-01-31"), "it is still the 31st here");
}
