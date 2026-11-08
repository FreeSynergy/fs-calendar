//! The task program in this process (`LocalTasks`) keeps what the calendar needs of it.

#![cfg(feature = "local")]

mod common;

use std::sync::Arc;

use fs_calendar_tasks::LocalTasks;

#[tokio::test]
async fn local_tasks_behave() {
    let (_store, program) = common::task_program().await;
    let local = Arc::new(LocalTasks::new(Arc::new(program.clone())));
    common::behaves(local.clone(), local, program).await;
}

#[tokio::test]
async fn a_task_whose_uuid_is_the_id_of_an_event_is_refused_not_called_known() {
    use fs_calendar_core::{Calendar, CalendarRepo, Event};
    use fs_objects::{Store, TursoStore};
    use fs_tasks_core::{TaskItems, TaskRepo};

    // One store for both programs, as the app has it.
    let store: Arc<dyn Store> = Arc::new(TursoStore::open_in_memory().await.unwrap());
    let tasks = TaskRepo::new(store.clone(), common::ME);
    let local = Arc::new(LocalTasks::new(Arc::new(tasks.clone())));
    let cal = CalendarRepo::new(store, common::ME).with_task_intake(local);
    let id = "0192f6a0-7c3e-7b1e-8a3d-5f6c7d8e9f01";
    let mut event = Event::new("Meeting", "2026-11-24T10:00:00", "Etc/UTC", "PT1H");
    event.uid = id.to_string();
    cal.create(event).await.unwrap();

    let task =
        format!(r#"{{"@type": "Task", "uid": "{id}", "version": "2.0", "title": "Same id"}}"#);
    let report = cal.import_jscalendar(&task).await.unwrap();
    assert_eq!(
        (report.created.len(), report.skipped.len()),
        (0, 0),
        "{report:?}"
    );
    assert!(report.failed[0].starts_with(id), "{report:?}");
    assert!(tasks.get_by_uid(id).await.unwrap().is_none());
}
