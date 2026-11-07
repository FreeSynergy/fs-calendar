//! A damaged event does not take the tasks along; a failing task source says so.
//!

mod common;

use common::{World, d, rich, rich_series};
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
fn tasks_are_read_on_their_own_when_an_event_is_damaged() {
    let w = World::berlin();
    w.add_json_past_core(damaged_series());
    w.tasks.add("Send the minutes", Some("2026-11-12T17:00:00"));
    let mut s = w.state();
    let item = s
        .items()
        .iter()
        .find(|i| i.title == "Send the minutes")
        .expect("the task is there")
        .key
        .clone();
    s.select(&item);
    assert!(matches!(s.shown(), Shown::Item { .. }), "{:?}", s.shown());
    assert_eq!(s.problems().len(), 1, "only the event: {:?}", s.problems());
}

#[test]
fn a_failing_task_source_is_what_is_said_and_the_events_stay() {
    let w = World::berlin();
    w.tasks.fail_with("the task store is away");
    w.add_json(rich());
    let read = w.adapter.items(d("2026-11-24"), d("2026-11-24")).unwrap();
    assert_eq!(read.items.len(), 1, "the event is there");
    assert_eq!(read.problems.len(), 1);
    assert!(
        read.problems[0].text.contains("task store is away"),
        "{:?}",
        read.problems
    );
    assert!(
        !read.problems[0].text.contains("damaged"),
        "{:?}",
        read.problems
    );
    let key = format!("task:{}@2026-11-24", fs_objects::ObjectId::new());
    let refused = w.adapter.read(&key).unwrap_err();
    assert_eq!(
        refused.kind,
        ErrorKind::Failed,
        "not \"no longer exists\": {refused}"
    );
}
