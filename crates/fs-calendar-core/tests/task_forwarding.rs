//! Tasks found in an import go to `fs-tasks`, whatever the format (ADR 0050).

mod common;

use std::sync::Arc;

use common::{ICS, setup};
use fs_calendar_core::CalendarRepo;
use fs_tasks_core::{TaskItems, TaskRepo};

const MIXED: &str = r#"[
  {"@type": "Event", "uid": "e1", "version": "2.0", "start": "2026-11-24T10:00:00", "title": "Meeting"},
  {"@type": "Task", "uid": "t1", "version": "2.0", "title": "Minutes", "percentComplete": 10},
  {"@type": "Task", "uid": "t2", "version": "2.0", "title": "Broken", "percentComplete": 200}
]"#;

async fn with_tasks() -> (CalendarRepo, TaskRepo) {
    let (r, store) = setup().await;
    let tasks = TaskRepo::new(store, common::ME);
    (r.with_tasks(Arc::new(tasks.clone())), tasks)
}

#[tokio::test]
async fn json_tasks_go_to_the_task_program() {
    let (cal, tasks) = with_tasks().await;
    let rep = cal.import_jscalendar(MIXED).await.unwrap();
    assert_eq!(rep.created.len(), 2, "one event and one task: {rep:?}");
    assert_eq!(cal.list_events().await.unwrap().len(), 1);
    let t1 = tasks.get_by_uid("t1").await.unwrap().unwrap();
    assert_eq!(t1.task.percent_complete, Some(10));
    assert!(tasks.get_by_uid("t2").await.unwrap().is_none());
    assert!(rep.failed[0].starts_with("t2:"), "{rep:?}");
}

#[tokio::test]
async fn json_tasks_are_not_imported_twice() {
    let (cal, _) = with_tasks().await;
    cal.import_jscalendar(MIXED).await.unwrap();
    let again = cal.import_jscalendar(MIXED).await.unwrap();
    assert_eq!(again.created.len(), 0, "{again:?}");
    assert!(again.skipped.contains(&"t1".to_string()), "{again:?}");
}

#[tokio::test]
async fn without_a_task_program_the_report_says_so() {
    let (cal, _) = setup().await;
    let json = cal.import_jscalendar(MIXED).await.unwrap();
    assert_eq!(json.created.len(), 1, "{json:?}");
    assert!(json.notes.iter().any(|n| n.starts_with("t1:")), "{json:?}");

    let (cal, _) = setup().await;
    let ics = cal.import_ics(ICS).await.unwrap();
    assert_eq!(ics.created.len(), 2, "{ics:?}");
    assert!(ics.failed.is_empty(), "{ics:?}");
    assert!(
        ics.notes.iter().any(|n| n.starts_with("todo-1:")),
        "{ics:?}"
    );
}
