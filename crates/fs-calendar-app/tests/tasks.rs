//! Tasks appear read-only through the `TaskItems` interface of `fs-tasks`, never owned here (ADR 0050).
//!
//! Pattern: Integration tests: a real `TaskRepo` over the same store.

mod common;

use common::{World, d};
use fs_components_workbenches::calendar::{CalendarSource, Shown};
use fs_tasks_core::{Task, TaskItems};
use serde_json::json;

fn add_task(w: &World, title: &str, due: &str) -> String {
    let mut task = Task::new(title);
    task.due = Some(due.to_string());
    task.time_zone = Some("Europe/Berlin".to_string());
    w.runtime
        .block_on(TaskItems::create(w.tasks.as_ref(), task))
        .unwrap()
        .id
        .to_string()
}

#[test]
fn a_task_is_shown_when_it_is_due_marked_as_a_task_and_cannot_be_changed_here() {
    let w = World::berlin();
    let id = add_task(&w, "Send the minutes", "2026-11-12T17:00:00");
    let item = w.item("2026-11-12", "Send the minutes");
    assert_eq!(item.start.clock(), "17:00");
    assert!(item.marks.contains(&"Task".to_string()));
    assert_eq!(item.key, format!("task:{id}@2026-11-12"));
    let mut s = w.state();
    assert!(s.items().iter().any(|i| i.key == item.key));
    s.select(&item.key);
    let Shown::Item { sheet, .. } = s.shown() else {
        panic!("{:?}", s.shown())
    };
    assert!(sheet.note.as_deref().unwrap().contains("tasks program"));
    assert!(!s.can_edit() && !s.can_delete());
    s.begin_edit();
    assert!(s.editing().is_none());
    s.ask_delete();
    assert!(!s.delete_asked());
    assert_eq!(
        w.adapter.delete(&item.key).unwrap_err().kind,
        fs_components_workbenches::ErrorKind::Unsupported
    );
    assert!(w.adapter.draft(&item.key).is_err());
    let task = w
        .runtime
        .block_on(TaskItems::get(w.tasks.as_ref(), id.parse().unwrap()))
        .unwrap();
    assert!(task.is_some(), "it stays in fs-tasks");
}

#[test]
fn a_task_due_outside_the_days_is_not_asked_about_and_an_undated_one_is_not_shown() {
    let w = World::berlin();
    add_task(&w, "Later", "2027-05-01T09:00:00");
    let undated = Task::new("Whenever");
    w.runtime
        .block_on(TaskItems::create(w.tasks.as_ref(), undated))
        .unwrap();
    assert!(w.items("2026-11-01", "2026-11-30").is_empty());
    let found = w.adapter.items(d("2027-05-01"), d("2027-05-01")).unwrap();
    assert_eq!(found.len(), 1);
}

#[test]
fn a_task_marked_sensitive_is_marked_here_too() {
    let w = World::berlin();
    let mut task = Task::new("Clinic form");
    task.due = Some("2026-11-12T17:00:00".to_string());
    task.time_zone = Some("Europe/Berlin".to_string());
    task.extra.insert(
        "freesynergy.net:sensitive".to_string(),
        json!(["pd:HealthRecord"]),
    );
    w.runtime
        .block_on(TaskItems::create(w.tasks.as_ref(), task))
        .unwrap();
    assert!(
        w.item("2026-11-12", "Clinic form")
            .marks
            .contains(&"Sensitive".to_string())
    );
}
