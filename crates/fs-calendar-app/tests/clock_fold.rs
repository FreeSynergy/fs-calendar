//! The hour that happens twice, when the clocks go back.
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

#[test]
fn an_event_in_the_hour_that_happens_twice_has_its_real_length_on_the_view() {
    let w = World::berlin();
    // 2026-10-25 in Berlin: 03:00 goes back to 02:00. 02:30 plus 30 minutes reads 02:00 on the clock.
    let key = w.add(timed(
        "Fold",
        "2026-10-25T02:30:00",
        "Europe/Berlin",
        "PT30M",
    ));
    let item = w.item("2026-10-25", "Fold");
    assert_eq!(item.start, m("2026-10-25", 2, 30));
    assert_eq!(
        item.end,
        m("2026-10-25", 3, 0),
        "half an hour drawn, not nothing"
    );
    let mut s = w.state();
    s.select(&key);
    let text = sheet_text(&s);
    assert!(
        text.contains("End: 2026-10-25 02:00 (after the clocks went back)"),
        "{text}"
    );
}

#[test]
fn a_zone_only_change_on_an_event_in_the_repeated_hour_is_not_refused_and_the_form_says_why() {
    let w = World::berlin();
    let key = w.add(timed(
        "Fold",
        "2026-10-25T02:30:00",
        "Europe/Berlin",
        "PT30M",
    ));
    let note = w.adapter.draft(&key).unwrap().note.unwrap_or_default();
    assert!(note.contains("clocks go back"), "{note}");
    let s = edit(&w, &key, |s| s.set_field("time_zone", "Asia/Tokyo"));
    assert!(
        s.editing().is_none(),
        "the end was not touched: {:?}",
        s.notice()
    );
    let stored = w.stored(&key).content;
    assert_eq!(stored["timeZone"], json!("Asia/Tokyo"));
    assert_eq!(stored["duration"], json!("PT30M"));
}
