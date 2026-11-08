//! Shared helpers of the integration tests: a real task program, and the behaviour every adapter must keep.

#![allow(dead_code)] // each test file uses a different subset

use std::sync::Arc;

use fs_calendar_core::{CalendarRepo, TaskIntake, TasksInWindow, utc};
use fs_objects::{Store, TursoStore};
use fs_tasks_core::{Task, TaskItems, TaskRepo};

pub const ME: &str = "https://local.invalid/users/owner";

pub const ICS: &str = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//x//y//EN\r\nBEGIN:VEVENT\r\nUID:ev-1\r\nDTSTAMP:20261101T000000Z\r\nDTSTART:20261112T090000Z\r\nDURATION:PT2H\r\nSUMMARY:Second\r\nEND:VEVENT\r\nBEGIN:VTODO\r\nUID:todo-1\r\nDTSTAMP:20261101T000000Z\r\nDUE:20261112T170000Z\r\nSUMMARY:Minutes\r\nSTATUS:NEEDS-ACTION\r\nPERCENT-COMPLETE:10\r\nEND:VTODO\r\nEND:VCALENDAR\r\n";

const MIXED: &str = r#"[
  {"@type": "Task", "uid": "t1", "version": "2.0", "title": "Minutes", "percentComplete": 10},
  {"@type": "Task", "uid": "t2", "version": "2.0", "title": "Broken", "percentComplete": 200}
]"#;

/// The task program: its store and a repository over it.
pub async fn task_program() -> (Arc<dyn Store>, TaskRepo) {
    let store: Arc<dyn Store> = Arc::new(TursoStore::open_in_memory().await.unwrap());
    let repo = TaskRepo::new(store.clone(), ME);
    (store, repo)
}

/// A calendar over a store of its own.
pub async fn calendar() -> CalendarRepo {
    let store: Arc<dyn Store> = Arc::new(TursoStore::open_in_memory().await.unwrap());
    CalendarRepo::new(store, ME)
}

/// A task due at `due` (a local date-time in UTC), or undated.
pub fn task(title: &str, due: Option<&str>) -> Task {
    let mut task = Task::new(title);
    task.due = due.map(str::to_string);
    task.time_zone = due.map(|_| "Etc/UTC".to_string());
    task
}

/// What every adapter over the task program must do, whatever the way to it: show the tasks due in a
/// window, hand over the tasks of an import once, refuse a bad one alone. `program` is the task
/// program itself, to look at what it holds.
pub async fn behaves(
    window: Arc<dyn TasksInWindow>,
    intake: Arc<dyn TaskIntake>,
    program: TaskRepo,
) {
    // Five tasks due in November, one in December, one undated; pages of two make three pages.
    for day in 1..=5 {
        let due = format!("2026-11-{day:02}T10:00:00");
        program
            .create(task(&format!("T{day}"), Some(&due)))
            .await
            .unwrap();
    }
    program
        .create(task("Later", Some("2026-12-24T10:00:00")))
        .await
        .unwrap();
    program.create(task("Undated", None)).await.unwrap();
    let (from, to) = (
        utc("2026-11-01T00:00:00Z").unwrap(),
        utc("2026-12-01T00:00:00Z").unwrap(),
    );
    let due = window.due_between(from, to).await.unwrap();
    let titles: Vec<_> = due
        .iter()
        .map(|t| t.jscalendar["title"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(titles, ["T1", "T2", "T3", "T4", "T5"]);
    assert_eq!(due[0].envelope.id, due[0].id);

    // The agenda of a calendar shows them next to its events.
    let cal = calendar()
        .await
        .with_tasks(window.clone())
        .with_task_intake(intake.clone());
    let agenda = cal.agenda(from, to).await.unwrap();
    assert_eq!((agenda.events.len(), agenda.tasks.len()), (0, 5));

    // An import: the VTODO of a text goes to the task program, once.
    let report = cal.import_ics(ICS).await.unwrap();
    assert_eq!(report.created.len(), 2, "an event and a task: {report:?}");
    assert!(program.get_by_uid("todo-1").await.unwrap().is_some());
    let again = cal.import_ics(ICS).await.unwrap();
    assert_eq!(
        (again.created.len(), again.skipped.len()),
        (0, 2),
        "{again:?}"
    );

    // A task in a JSCalendar file: one is stored, one is refused and named, nothing else is lost.
    let report = cal.import_jscalendar(MIXED).await.unwrap();
    assert_eq!(report.created.len(), 1, "{report:?}");
    assert!(report.failed[0].starts_with("t2:"), "{report:?}");
    assert!(program.get_by_uid("t1").await.unwrap().is_some());
    assert!(program.get_by_uid("t2").await.unwrap().is_none());
    let again = cal.import_jscalendar(MIXED).await.unwrap();
    assert!(again.skipped.contains(&"t1".to_string()), "{again:?}");
    assert_eq!(
        cal.list_events().await.unwrap().len(),
        1,
        "only the event of the ics"
    );
}
