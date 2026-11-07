//! One bad event costs itself only; an override moved far back is found; what cannot be stored is
//! refused without a panic.
//!
//! Pattern: Integration tests over the real core, with what the core lets in.

mod common;

use common::{World, d, rich, rich_series, sheet_text};
use fs_calendar_core::Event;
use fs_components_workbenches::ErrorKind;
use fs_components_workbenches::calendar::{CalendarSource, Shown};
use serde_json::json;

/// An endless series with an override the core stores and cannot expand.
fn damaged_series() -> serde_json::Value {
    let mut event = rich_series();
    event["title"] = json!("Bad series");
    event["recurrenceRule"] = json!({"@type": "RecurrenceRule", "frequency": "weekly"});
    event["recurrenceOverrides"] = json!({"2026-11-10T10:00:00": {"duration": "nonsense"}});
    event
}

#[test]
fn one_bad_override_leaves_the_rest_shown_and_is_named_and_deletable() {
    let w = World::berlin();
    let bad = w.add_json_past_core(damaged_series());
    w.add_json(rich());
    let core = w
        .runtime
        .block_on(w.repo.window(
            fs_calendar_core::utc("2026-11-01T00:00:00Z").unwrap(),
            fs_calendar_core::utc("2026-12-01T00:00:00Z").unwrap(),
        ))
        .unwrap();
    assert_eq!(core.damaged.len(), 1, "the core names the damage: {core:?}");
    let mut s = w.state();
    assert_eq!(s.problem(), None, "the calendar is not down");
    assert!(
        s.items().iter().any(|i| i.title == "Kick-off"),
        "the rest is there"
    );
    let named = &s.problems()[0];
    assert!(named.text.contains("Bad series"), "{}", named.text);
    assert_eq!(named.key.as_deref(), Some(bad.as_str()));
    let key = named.key.clone().unwrap();
    s.select(&key);
    assert!(sheet_text(&s).contains("Bad series"), "{:?}", s.shown());
    assert!(s.can_delete());
    s.ask_delete();
    assert!(s.notice().unwrap().contains("all its occurrences go"));
    s.confirm_delete();
    assert!(
        s.problems().is_empty(),
        "gone, and the calendar is whole again"
    );
    assert!(!s.items().is_empty());
}

#[test]
fn an_occurrence_of_a_series_gone_bad_since_says_so_and_warns_before_a_delete() {
    let w = World::berlin();
    let mut event = rich_series();
    event["recurrenceRule"] = json!({"@type": "RecurrenceRule", "frequency": "weekly"});
    let key = w.add_json(event.clone());
    let occurrence = format!("{key}#2026-11-24T10:00:00");
    w.put_overrides(
        &key,
        json!({"2026-11-10T10:00:00": {"duration": "nonsense"}}),
    );
    let mut s = w.state();
    s.select(&occurrence);
    assert!(matches!(s.shown(), Shown::Failed { .. }), "{:?}", s.shown());
    s.ask_delete();
    assert!(
        s.notice().unwrap().contains("all its occurrences go"),
        "{:?}",
        s.notice()
    );
}

#[test]
fn an_override_moved_before_the_series_is_shown_where_it_went_and_opens() {
    let w = World::berlin();
    let mut event = rich_series();
    event["recurrenceOverrides"] = json!({
        "2026-11-10T10:00:00": {"title": "Moved far back", "start": "2026-09-01T10:00:00"}
    });
    let key = w.add_json(event);
    let moved = w.item("2026-09-01", "Moved far back");
    assert_eq!(moved.key, format!("{key}#2026-11-10T10:00:00"));
    assert_eq!(w.items("2026-09-01", "2026-09-07").len(), 1, "in its week");
    assert!(
        w.items("2026-11-10", "2026-11-10").is_empty(),
        "not where it was"
    );
    assert_eq!(
        w.items("2026-11-03", "2026-11-03").len(),
        1,
        "and only once where the rest of the series is"
    );
    let mut s = w.state();
    s.select(&moved.key);
    assert!(sheet_text(&s).contains("Moved far back"), "{:?}", s.shown());
    let mut wide = w.state();
    wide.go_to(d("2026-09-15"));
    wide.select(&moved.key);
    assert!(
        matches!(wide.shown(), Shown::Item { .. }),
        "{:?}",
        wide.shown()
    );
}

#[test]
fn a_bad_key_of_a_task_or_event_is_not_found() {
    let w = World::berlin();
    for key in ["task:nope", "task:x@2026-11-06", "not-an-id#x"] {
        assert_eq!(
            w.adapter.read(key).unwrap_err().kind,
            ErrorKind::NotFound,
            "{key}"
        );
    }
}

#[test]
fn a_damaged_event_marked_sensitive_is_not_named_and_carries_its_mark() {
    let w = World::berlin();
    let mut event = damaged_series();
    event["title"] = json!("Clinic");
    event["freesynergy.net:sensitive"] = json!(["pd:HealthRecord"]);
    w.add_json_past_core(event);
    let s = w.state();
    let problem = &s.problems()[0];
    assert!(!problem.text.contains("Clinic"), "{}", problem.text);
    assert_eq!(problem.marks, ["Sensitive"]);
}

/// An event with a length no calculation can hold, written past the core, as an old store might have it.
fn too_long_in_the_store(w: &World) -> String {
    let key = w.add_json(rich());
    let mut object = w.stored(&key);
    object.content["duration"] = json!("P99999999999D");
    object.content["title"] = json!("Too long");
    object.envelope.name = "Too long".to_string();
    w.runtime.block_on(w.store.put(object, common::ME)).unwrap();
    key
}

#[test]
fn a_length_too_long_in_the_store_lets_the_window_start_and_names_it_and_it_can_go() {
    let w = World::berlin();
    w.add(Event::new(
        "Fine",
        "2026-11-25T10:00:00",
        "Europe/Berlin",
        "PT1H",
    ));
    let key = too_long_in_the_store(&w);
    w.item("2026-11-25", "Fine");
    let mut s = w.state();
    assert!(s.items().iter().any(|i| i.title == "Fine"));
    assert!(
        s.problems().iter().any(|p| p.text.contains("Too long")),
        "{:?}",
        s.problems()
    );
    s.select(&key);
    assert!(matches!(s.shown(), Shown::Failed { .. }), "{:?}", s.shown());
    s.ask_delete();
    s.confirm_delete();
    assert!(s.problems().is_empty(), "{:?}", s.problems());
}
