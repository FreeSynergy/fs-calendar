//! Occurrences before a series' first start: found at each reading, right at once.
//!
//! Pattern: Integration tests over the real core; changes made past the adapter, as elsewhere.

mod common;

use common::{World, d, rich_series, sheet_text};
use fs_calendar_core::Event;
use fs_components_workbenches::calendar::CalendarSource;
use fs_objects::ObjectId;
use serde_json::json;

#[test]
fn an_early_series_changed_or_gone_elsewhere_is_right_at_once() {
    let w = World::berlin();
    let mut event = rich_series();
    event["recurrenceOverrides"] = json!({
        "2026-11-10T10:00:00": {"title": "Back", "start": "2026-09-01T10:00:00"}
    });
    let key = w.add_json(event.clone());
    assert_eq!(w.item("2026-09-01", "Back").start.clock(), "10:00");
    event["recurrenceOverrides"] = json!({
        "2026-11-10T10:00:00": {"title": "Back", "start": "2026-09-01T15:00:00"}
    });
    let id: ObjectId = key.parse().unwrap();
    w.runtime
        .block_on(w.repo.update(id, Event::from_value(event.clone()).unwrap()))
        .unwrap();
    assert_eq!(
        w.item("2026-09-01", "Back").start.clock(),
        "15:00",
        "the new time"
    );
    w.runtime.block_on(w.repo.delete(id)).unwrap();
    assert_eq!(
        w.items("2026-09-01", "2026-09-07"),
        [] as [fs_components_workbenches::calendar::CalendarItem; 0]
    );
    let mut s = w.state();
    s.go_to(d("2026-09-01"));
    assert!(
        s.problems().is_empty(),
        "gone, without a word: {:?}",
        s.problems()
    );
    let later = w.add_json(rich_series());
    assert_eq!(
        w.items("2026-09-01", "2026-09-07"),
        [] as [fs_components_workbenches::calendar::CalendarItem; 0]
    );
    let mut moved = w.stored(&later).content;
    moved["recurrenceOverrides"] = json!({
        "2026-11-10T10:00:00": {"title": "Became early", "start": "2026-09-02T10:00:00"}
    });
    w.runtime
        .block_on(
            w.repo
                .update(later.parse().unwrap(), Event::from_value(moved).unwrap()),
        )
        .unwrap();
    assert_eq!(
        w.items("2026-09-02", "2026-09-02").len(),
        1,
        "seen without a write of our own"
    );
}

#[test]
fn an_extra_occurrence_before_the_first_start_is_in_its_own_week() {
    let w = World::berlin();
    let mut event = rich_series();
    event["recurrenceOverrides"] = json!({"2026-09-01T10:00:00": {"title": "Extra"}});
    let key = w.add_json(event);
    let item = w.item("2026-09-01", "Extra");
    assert_eq!(item.key, format!("{key}#2026-09-01T10:00:00"));
    let mut s = w.state();
    s.select(&item.key);
    assert!(sheet_text(&s).contains("Extra"), "{:?}", s.shown());
}

/// A measurement, not a check: `cargo test --release -- --ignored --nocapture looking_for`.
#[test]
#[ignore = "takes minutes in a debug build"]
fn looking_for_early_series_at_each_reading_costs_this() {
    let w = World::berlin();
    for i in 0..5000u32 {
        let mut event = Event::new(
            format!("Event {i}"),
            "2026-11-25T10:00:00",
            "Europe/Berlin",
            "PT1H",
        );
        event.uid = format!("urn:uuid:01a1aaf0-08a0-756c-85b2-f0927{i:07x}");
        w.add(event);
    }
    let started = std::time::Instant::now();
    let read = w.adapter.items(d("2026-12-20"), d("2026-12-26")).unwrap();
    eprintln!(
        "a week among 5000 events: {} items in {:?}",
        read.items.len(),
        started.elapsed()
    );
    assert!(started.elapsed().as_secs() < 5, "{:?}", started.elapsed());
}
