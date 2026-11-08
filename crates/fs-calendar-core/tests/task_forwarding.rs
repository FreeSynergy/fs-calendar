//! Tasks found in an import go to the task program, whatever the format (ADR 0050). The task program is
//! a fake behind `TaskIntake`.

mod common;

use std::sync::Arc;

use common::fake_tasks::FakeTasks;
use common::{ICS, setup};
use fs_calendar_core::CalendarRepo;

const MIXED: &str = r#"[
  {"@type": "Event", "uid": "e1", "version": "2.0", "start": "2026-11-24T10:00:00", "title": "Meeting"},
  {"@type": "Task", "uid": "t1", "version": "2.0", "title": "Minutes", "percentComplete": 10},
  {"@type": "Task", "uid": "t2", "version": "2.0", "title": "Broken", "percentComplete": 200}
]"#;

async fn with_intake() -> (CalendarRepo, Arc<FakeTasks>) {
    let (r, _) = setup().await;
    let fake = Arc::new(FakeTasks::default());
    (r.with_task_intake(fake.clone()), fake)
}

#[tokio::test]
async fn json_tasks_go_to_the_task_program() {
    let (cal, fake) = with_intake().await;
    let rep = cal.import_jscalendar(MIXED).await.unwrap();
    assert_eq!(rep.created.len(), 2, "one event and one task: {rep:?}");
    assert_eq!(cal.list_events().await.unwrap().len(), 1);
    assert_eq!(*fake.uids.lock().unwrap(), ["t1"]);
    assert!(rep.failed[0].starts_with("t2: percentComplete"), "{rep:?}");
    // the task arrives as the text it was found in
    assert!(fake.texts.lock().unwrap()[0].contains("\"Minutes\""));
}

#[tokio::test]
async fn json_tasks_are_not_imported_twice() {
    let (cal, _) = with_intake().await;
    cal.import_jscalendar(MIXED).await.unwrap();
    let again = cal.import_jscalendar(MIXED).await.unwrap();
    assert_eq!(again.created.len(), 0, "{again:?}");
    assert!(again.skipped.contains(&"t1".to_string()), "{again:?}");
}

#[tokio::test]
async fn a_task_without_a_uid_is_named_not_forwarded() {
    let (cal, fake) = with_intake().await;
    let rep = cal
        .import_jscalendar(r#"{"@type": "Task", "version": "2.0", "title": "No uid"}"#)
        .await
        .unwrap();
    assert_eq!(rep.failed, ["component without uid"]);
    assert!(fake.uids.lock().unwrap().is_empty());
}

#[tokio::test]
async fn ics_vtodos_go_to_the_task_program_as_text() {
    let (cal, fake) = with_intake().await;
    let rep = cal.import_ics(ICS).await.unwrap();
    assert_eq!(rep.created.len(), 3, "two events and one task: {rep:?}");
    assert_eq!(*fake.uids.lock().unwrap(), ["todo-1"]);
    assert_eq!(cal.list_events().await.unwrap().len(), 2);
    let again = cal.import_ics(ICS).await.unwrap();
    assert_eq!((again.created.len(), again.skipped.len()), (0, 3));
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

#[tokio::test]
async fn the_uid_goes_to_the_task_program_as_written() {
    let (cal, fake) = with_intake().await;
    let json = r#"{"@type": "Task", "uid": " Todo-X ", "version": "2.0", "title": "Padded"}"#;
    cal.import_jscalendar(json).await.unwrap();
    assert_eq!(*fake.uids.lock().unwrap(), [" Todo-X "]);
    let again = cal.import_jscalendar(json).await.unwrap();
    assert_eq!(again.skipped, [" Todo-X "], "{again:?}");
}

#[tokio::test]
async fn a_task_program_that_is_gone_loses_its_tasks_named_and_the_events_stay() {
    let (r, _) = setup().await;
    let fake = Arc::new(FakeTasks {
        broken: true,
        ..FakeTasks::default()
    });
    let cal = r.with_task_intake(fake);
    let json = cal.import_jscalendar(MIXED).await.unwrap();
    assert_eq!(json.created.len(), 1, "the event is stored: {json:?}");
    assert_eq!(json.failed.len(), 2, "{json:?}");
    assert!(
        json.failed[0].starts_with("t1: the task program is gone"),
        "{json:?}"
    );
    assert_eq!(cal.list_events().await.unwrap().len(), 1);
}
