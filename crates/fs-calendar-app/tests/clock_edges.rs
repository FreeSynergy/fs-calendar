//! Times that do not exist or happen twice; day counts of all-day events.
//!
//! Table tests through the real server and client.

mod common;

use common::{World, d, sheet_text};
use fs_calendar_core::Event;
use fs_components_workbenches::calendar::{CalendarSource, CalendarState, Moment};
use serde_json::json;

fn timed(title: &str, start: &str, zone: &str, duration: &str) -> Event {
    Event::new(title, start, zone, duration)
}

fn m(day: &str, hour: u16, minute: u16) -> Moment {
    Moment::at(d(day), hour, minute)
}

fn edit(w: &World, key: &str, change: impl Fn(&mut CalendarState)) -> CalendarState {
    let mut s = w.state();
    s.select(key);
    s.begin_edit();
    change(&mut s);
    s.save();
    s
}

fn error_of(s: &CalendarState) -> String {
    s.editing()
        .and_then(|e| e.error.clone())
        .unwrap_or_default()
}

#[test]
fn all_day_lengths_in_hours_count_whole_days() {
    let w = World::berlin();
    for (i, (length, days, last)) in [
        ("P1D", 1, "2026-11-24"),
        ("PT24H", 1, "2026-11-24"),
        ("PT25H", 2, "2026-11-25"),
        ("PT48H", 2, "2026-11-25"),
        ("P1DT24H", 2, "2026-11-25"),
        ("P1DT1H", 2, "2026-11-25"),
        ("PT0S", 1, "2026-11-24"),
    ]
    .into_iter()
    .enumerate()
    {
        let title = format!("Days {i}");
        let mut event = timed(&title, "2026-11-24T00:00:00", "Etc/UTC", length);
        event.time_zone = None;
        event.show_without_time = Some(true);
        let key = w.add(event);
        let item = w.item("2026-11-24", &title);
        assert_eq!(
            item.end,
            Moment::start_of(d("2026-11-24").add_days(days)),
            "{length}"
        );
        assert_eq!(
            w.adapter.draft(&key).unwrap().value("end"),
            Some(last),
            "{length}"
        );
    }
}

#[test]
fn a_timed_event_ending_at_midnight_turned_all_day_is_its_one_day() {
    let w = World::berlin();
    let key = w.add(timed(
        "Evening",
        "2026-11-24T22:00:00",
        "Europe/Berlin",
        "PT2H",
    ));
    let s = edit(&w, &key, |s| s.toggle_field("all_day"));
    assert!(s.editing().is_none(), "{:?}", s.notice());
    assert_eq!(w.stored(&key).content["duration"], json!("P1D"));
    assert!(w.items("2026-11-25", "2026-11-25").is_empty());
}

#[test]
fn a_time_the_clocks_skip_is_refused_and_nothing_is_stored() {
    let w = World::berlin();
    // 2027-03-28 in Berlin: 02:00 jumps to 03:00.
    let key = w.add(timed(
        "Spring",
        "2027-03-28T01:00:00",
        "Europe/Berlin",
        "PT30M",
    ));
    let before = w.stored(&key).content;
    let s = edit(&w, &key, |s| {
        s.set_field("start", "2027-03-28 02:30");
        s.set_field("end", "2027-03-28 03:00");
    });
    assert!(error_of(&s).contains("does not exist"), "{}", error_of(&s));
    assert!(error_of(&s).contains("02:30"));
    let s = edit(&w, &key, |s| s.set_field("end", "2027-03-28 02:30"));
    assert!(error_of(&s).contains("does not exist"), "{}", error_of(&s));
    assert_eq!(w.stored(&key).content, before);
    let s = edit(&w, &key, |s| s.set_field("end", "2027-03-28 03:30"));
    assert!(s.editing().is_none(), "{:?}", s.notice());
    assert_eq!(
        w.stored(&key).content["duration"],
        json!("PT1H30M"),
        "real time across the gap"
    );
    assert_eq!(w.item("2027-03-28", "Spring").end, m("2027-03-28", 3, 30));
}

#[test]
fn a_start_in_the_gap_from_elsewhere_reads_as_what_is_drawn() {
    let w = World::berlin();
    let key = w.add(timed(
        "From a file",
        "2027-03-28T02:15:00",
        "Europe/Berlin",
        "PT30M",
    ));
    let drawn = w.item("2027-03-28", "From a file");
    assert_eq!(drawn.start, m("2027-03-28", 3, 15));
    let draft = w.adapter.draft(&key).unwrap();
    assert_eq!(draft.value("start"), Some("2027-03-28 03:15"), "as drawn");
    let mut s = w.state();
    s.select(&key);
    assert!(
        sheet_text(&s).contains("Start: 2027-03-28 03:15"),
        "{}",
        sheet_text(&s)
    );
    let s = edit(&w, &key, |s| s.set_field("title", "Renamed"));
    assert!(s.editing().is_none());
    assert_eq!(
        w.stored(&key).content["start"],
        json!("2027-03-28T02:15:00")
    );
}

#[test]
fn changing_only_the_zone_keeps_both_clock_times_across_a_clock_change() {
    let w = World::berlin();
    // Five hours from 00:30 end at 05:30 in Tokyo; in Berlin that night has 25 hours.
    let key = w.add(timed("Night", "2026-10-25T00:30:00", "Asia/Tokyo", "PT5H"));
    let s = edit(&w, &key, |s| s.set_field("time_zone", "Europe/Berlin"));
    assert!(s.editing().is_none(), "{:?}", s.notice());
    let stored = w.stored(&key).content;
    assert_eq!(stored["start"], json!("2026-10-25T00:30:00"));
    assert_eq!(
        stored["duration"],
        json!("PT6H"),
        "six real hours to the same clock"
    );
    assert_eq!(
        w.adapter.draft(&key).unwrap().value("end"),
        Some("2026-10-25 05:30")
    );
    // A length that crosses nothing is not rewritten.
    let plain = w.add(timed("Plain", "2026-11-25T10:00:00", "Asia/Tokyo", "P1D"));
    let s = edit(&w, &plain, |s| s.set_field("time_zone", "Europe/Berlin"));
    assert!(s.editing().is_none());
    assert_eq!(w.stored(&plain).content["duration"], json!("P1D"));
}

#[test]
fn a_zone_where_the_start_does_not_exist_is_refused() {
    let w = World::berlin();
    let key = w.add(timed("Gap", "2027-03-28T02:30:00", "Asia/Tokyo", "PT1H"));
    let before = w.stored(&key).content;
    let s = edit(&w, &key, |s| s.set_field("time_zone", "Europe/Berlin"));
    assert!(error_of(&s).contains("does not exist"), "{}", error_of(&s));
    assert_eq!(w.stored(&key).content, before);
}
