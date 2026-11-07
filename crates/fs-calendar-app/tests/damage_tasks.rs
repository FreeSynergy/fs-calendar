//! A damaged event does not take the tasks along; a failing task source says so.
//!
//! Pattern: Integration tests over the real core, and a task source that fails on purpose.

mod common;

use std::sync::Arc;

use async_trait::async_trait;
use common::{World, d, rich, rich_series};
use fs_calendar_app::CalendarAdapter;
use fs_calendar_core::CalendarRepo;
use fs_components_workbenches::ErrorKind;
use fs_components_workbenches::calendar::{CalendarSource, Shown};
use fs_objects::ObjectId;
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
fn tasks_are_read_on_their_own_when_an_event_is_damaged() {
    let w = World::berlin();
    w.add_json_past_core(damaged_series());
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
    fn capabilities(&self) -> fs_tasks_core::Capabilities {
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
    async fn page(
        &self,
        _: &TaskQuery,
        _: Option<&fs_tasks_core::Place>,
        _: usize,
    ) -> fs_tasks_core::Result<fs_tasks_core::TaskPage> {
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
