//! The calendar shows tasks only through the `fs-tasks` interface (ADR 0050).

mod common;

use std::sync::Arc;

use async_trait::async_trait;
use common::{ICS, setup, t};
use fs_calendar_core::{Calendar, CalendarRepo, Event};
use fs_objects::{Envelope, ObjectId};
use fs_tasks_core::{
    Capabilities, ImportReport, Place, Relation, Result, Task, TaskEntry, TaskItems, TaskPage,
    TaskQuery, TaskRepo,
};

/// A task source that answers every query with one canned task and records the query.
#[derive(Default)]
struct Canned(std::sync::Mutex<Vec<TaskQuery>>);

#[async_trait]
impl TaskItems for Canned {
    fn capabilities(&self) -> Capabilities {
        Capabilities::new("canned")
    }
    async fn create(&self, task: Task) -> Result<TaskEntry> {
        Ok(TaskEntry {
            id: ObjectId::new(),
            envelope: Envelope::new("Task", task.display_title()),
            task,
        })
    }
    async fn edit(&self, id: ObjectId, task: Task) -> Result<TaskEntry> {
        Ok(TaskEntry {
            id,
            envelope: Envelope::new("Task", task.display_title()),
            task,
        })
    }
    async fn assign(&self, id: ObjectId, _: &str, _: &str) -> Result<TaskEntry> {
        self.edit(id, Task::new("x")).await
    }
    async fn relate(&self, id: ObjectId, _: &str, _: Relation) -> Result<TaskEntry> {
        self.edit(id, Task::new("x")).await
    }
    async fn label(&self, id: ObjectId, _: &str) -> Result<TaskEntry> {
        self.edit(id, Task::new("x")).await
    }
    async fn get(&self, _: ObjectId) -> Result<Option<TaskEntry>> {
        Ok(None)
    }
    async fn get_by_uid(&self, _: &str) -> Result<Option<TaskEntry>> {
        Ok(None)
    }
    async fn delete(&self, _: ObjectId) -> Result<()> {
        Ok(())
    }
    async fn list(&self) -> Result<Vec<TaskEntry>> {
        Ok(vec![])
    }
    async fn query(&self, q: &TaskQuery) -> Result<Vec<TaskEntry>> {
        self.0.lock().unwrap().push(q.clone());
        Ok(vec![TaskEntry {
            id: ObjectId::new(),
            envelope: Envelope::new("Task", "Canned"),
            task: Task::new("Canned"),
        }])
    }
    async fn page(&self, _: &TaskQuery, _: Option<&Place>, _: usize) -> Result<TaskPage> {
        Ok(TaskPage::default())
    }
    async fn import_ics(&self, _: &str) -> Result<ImportReport> {
        Ok(ImportReport::default())
    }
    async fn export_ics(&self, _: &[ObjectId]) -> Result<String> {
        Ok(String::new())
    }
}

#[tokio::test]
async fn agenda_asks_the_task_interface_for_the_window() {
    let canned = Arc::new(Canned::default());
    let (r, _) = setup().await;
    let cal: Arc<dyn Calendar> = Arc::new(r.with_tasks(canned.clone()));
    cal.create(Event::new(
        "Planning",
        "2026-11-24T10:00:00",
        "Etc/UTC",
        "PT1H",
    ))
    .await
    .unwrap();
    let (from, to) = (t("2026-11-01T00:00:00Z"), t("2026-12-01T00:00:00Z"));
    let agenda = cal.agenda(from, to).await.unwrap();
    assert_eq!(agenda.events.len(), 1);
    assert_eq!(agenda.tasks.len(), 1);
    assert_eq!(agenda.tasks[0].task.title.as_deref(), Some("Canned"));
    assert_eq!(
        *canned.0.lock().unwrap(),
        [TaskQuery::new().due_between(from, to)]
    );
}

#[tokio::test]
async fn without_a_task_source_the_agenda_has_only_events() {
    let (r, _) = setup().await;
    r.create(Event::new(
        "Planning",
        "2026-11-24T10:00:00",
        "Etc/UTC",
        "PT1H",
    ))
    .await
    .unwrap();
    let agenda = r
        .agenda(t("2026-11-01T00:00:00Z"), t("2026-12-01T00:00:00Z"))
        .await
        .unwrap();
    assert_eq!((agenda.events.len(), agenda.tasks.len()), (1, 0));
}

#[tokio::test]
async fn events_and_real_tasks_share_a_store_but_not_a_model() {
    let (cal, store) = setup().await;
    let tasks = TaskRepo::new(store, common::ME);
    let cal: CalendarRepo = cal.with_tasks(Arc::new(tasks.clone()));
    let mut task = Task::new("Write the minutes");
    task.due = Some("2026-11-12T17:00:00".into());
    task.time_zone = Some("Etc/UTC".into());
    tasks.create(task).await.unwrap();
    cal.create(Event::new(
        "Jour fixe",
        "2026-11-12T10:00:00",
        "Etc/UTC",
        "PT1H",
    ))
    .await
    .unwrap();
    let agenda = cal
        .agenda(t("2026-11-12T00:00:00Z"), t("2026-11-13T00:00:00Z"))
        .await
        .unwrap();
    assert_eq!(
        agenda.events[0].occurrence.event.display_title(),
        "Jour fixe"
    );
    assert_eq!(
        agenda.tasks[0].task.title.as_deref(),
        Some("Write the minutes")
    );
    // the calendar neither lists nor returns tasks as events
    assert_eq!(cal.list_events().await.unwrap().len(), 1);
    assert!(cal.get(agenda.tasks[0].id).await.unwrap().is_none());
}

#[tokio::test]
async fn ics_vtodos_go_to_the_task_source() {
    let (r, store) = setup().await;
    let tasks = TaskRepo::new(store, common::ME);
    let cal = r.with_tasks(Arc::new(tasks.clone()));
    let rep = cal.import_ics(ICS).await.unwrap();
    assert_eq!(rep.created.len(), 3, "two events and one task: {rep:?}");
    let todo = tasks.get_by_uid("todo-1").await.unwrap().unwrap();
    assert_eq!(todo.task.percent_complete, Some(10));
    assert_eq!(cal.list_events().await.unwrap().len(), 2);
    let again = cal.import_ics(ICS).await.unwrap();
    assert_eq!((again.created.len(), again.skipped.len()), (0, 3));
}
