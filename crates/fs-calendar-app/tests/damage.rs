//! One bad event costs itself only; an override moved far back is found; what cannot be stored is
//! refused without a panic.
//!
//! Pattern: Integration tests over the real core, with what the core lets in.

mod common;

use std::sync::Arc;

use async_trait::async_trait;
use common::{World, d, rich, rich_series, sheet_text};
use fs_calendar_app::CalendarAdapter;
use fs_calendar_core::{CalendarRepo, Event};
use fs_components_workbenches::ErrorKind;
use fs_components_workbenches::calendar::{CalendarSource, Shown};
use fs_store::ObjectId;
use fs_tasks_core::{Task, TaskEntry, TaskItems, TaskQuery};
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
    let bad = w.add_json(damaged_series());
    w.add_json(rich());
    let core = w.runtime.block_on(w.repo.occurrences(
        fs_calendar_core::utc("2026-11-01T00:00:00Z").unwrap(),
        fs_calendar_core::utc("2026-12-01T00:00:00Z").unwrap(),
    ));
    assert!(
        core.is_err(),
        "the core fails as a whole: that is the case here"
    );
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
    event["recurrenceOverrides"] = json!({"2026-11-10T10:00:00": {"duration": "nonsense"}});
    w.runtime
        .block_on(
            w.repo
                .update(key.parse().unwrap(), Event::from_value(event).unwrap()),
        )
        .unwrap();
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
    w.add_json(event);
    let s = w.state();
    let problem = &s.problems()[0];
    assert!(!problem.text.contains("Clinic"), "{}", problem.text);
    assert_eq!(problem.marks, ["Sensitive"]);
}

#[test]
fn tasks_are_read_on_their_own_when_an_event_is_damaged() {
    let w = World::berlin();
    w.add_json(damaged_series());
    let mut task = Task::new("Send the minutes");
    task.due = Some("2026-11-12T17:00:00".to_string());
    task.time_zone = Some("Europe/Berlin".to_string());
    w.runtime
        .block_on(TaskItems::create(w.tasks.as_ref(), task))
        .unwrap();
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

/// A task source that fails, to see what is said.
struct NoTasks;

#[async_trait]
impl TaskItems for NoTasks {
    fn capabilities(&self) -> fs_calendar_core::Capabilities {
        unimplemented!("not asked here")
    }
    async fn get(&self, _: ObjectId) -> fs_tasks_core::Result<Option<TaskEntry>> {
        Ok(None)
    }
    async fn get_by_uid(&self, _: &str) -> fs_tasks_core::Result<Option<TaskEntry>> {
        Ok(None)
    }
    async fn list(&self) -> fs_tasks_core::Result<Vec<TaskEntry>> {
        Ok(Vec::new())
    }
    async fn query(&self, _: &TaskQuery) -> fs_tasks_core::Result<Vec<TaskEntry>> {
        Err(fs_tasks_core::Error::Time("the task store is away".into()))
    }
}

#[test]
fn a_failing_task_source_is_what_is_said_and_the_events_stay() {
    let w = World::berlin();
    let source: Arc<dyn TaskItems> = Arc::new(NoTasks);
    let store = w.store.clone();
    let repo = Arc::new(CalendarRepo::new(store, common::ME).with_tasks(source.clone()));
    w.add_json(rich());
    let adapter = CalendarAdapter::new(
        repo,
        w.runtime.handle().clone(),
        fs_calendar_core::Zone::parse("Europe/Berlin").unwrap(),
    )
    .with_tasks(source);
    let read = adapter.items(d("2026-11-24"), d("2026-11-24")).unwrap();
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
    let key = format!("task:{}@2026-11-24", ObjectId::new());
    let refused = adapter.read(&key).unwrap_err();
    assert_eq!(
        refused.kind,
        ErrorKind::Failed,
        "not \"no longer exists\": {refused}"
    );
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
    assert!(w.items("2026-09-01", "2026-09-07").is_empty());
    let mut s = w.state();
    s.go_to(d("2026-09-01"));
    assert!(
        s.problems().is_empty(),
        "gone, without a word: {:?}",
        s.problems()
    );
    let later = w.add_json(rich_series());
    assert!(w.items("2026-09-01", "2026-09-07").is_empty());
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
