//! The calendar shows tasks only through the small trait `TasksInWindow` (ADR 0050): it knows no task
//! type and no task crate. The task program here is a fake; the real one is tested in `fs-calendar-tasks`.

mod common;

use std::sync::Arc;

use common::fake_tasks::FakeTasks;
use common::{setup, t};
use fs_calendar_core::{Calendar, Error, Event};

fn planning() -> Event {
    Event::new("Planning", "2026-11-24T10:00:00", "Etc/UTC", "PT1H")
}

#[tokio::test]
async fn agenda_asks_for_the_tasks_due_in_the_window() {
    let fake = Arc::new(FakeTasks {
        due: vec![FakeTasks::task("Canned")],
        ..FakeTasks::default()
    });
    let (r, _) = setup().await;
    let cal: Arc<dyn Calendar> = Arc::new(r.with_tasks(fake.clone()));
    cal.create(planning()).await.unwrap();
    let (from, to) = (t("2026-11-01T00:00:00Z"), t("2026-12-01T00:00:00Z"));
    let agenda = cal.agenda(from, to).await.unwrap();
    assert_eq!(agenda.events.len(), 1);
    assert_eq!(agenda.tasks.len(), 1);
    assert_eq!(agenda.tasks[0].jscalendar["title"], "Canned");
    assert_eq!(agenda.tasks[0].envelope.name, "Canned");
    assert_eq!(*fake.windows.lock().unwrap(), [(from, to)]);
}

#[tokio::test]
async fn without_a_task_program_the_agenda_has_only_events() {
    let (r, _) = setup().await;
    r.create(planning()).await.unwrap();
    let agenda = r
        .agenda(t("2026-11-01T00:00:00Z"), t("2026-12-01T00:00:00Z"))
        .await
        .unwrap();
    assert_eq!((agenda.events.len(), agenda.tasks.len()), (1, 0));
}

#[tokio::test]
async fn a_task_program_that_fails_is_an_error_named_tasks() {
    let fake = Arc::new(FakeTasks {
        broken: true,
        ..FakeTasks::default()
    });
    let (r, _) = setup().await;
    let cal = r.with_tasks(fake);
    cal.create(planning()).await.unwrap();
    let failed = cal
        .agenda(t("2026-11-01T00:00:00Z"), t("2026-12-01T00:00:00Z"))
        .await
        .unwrap_err();
    assert!(matches!(failed, Error::Tasks(_)), "{failed:?}");
    assert_eq!(failed.to_string(), "tasks: the task program is gone");
}

#[tokio::test]
async fn tasks_are_neither_listed_nor_returned_as_events() {
    let fake = Arc::new(FakeTasks {
        due: vec![FakeTasks::task("Write the minutes")],
        ..FakeTasks::default()
    });
    let (r, _) = setup().await;
    let cal = r.with_tasks(fake);
    cal.create(planning()).await.unwrap();
    let agenda = cal
        .agenda(t("2026-11-01T00:00:00Z"), t("2026-12-01T00:00:00Z"))
        .await
        .unwrap();
    assert_eq!(cal.list_events().await.unwrap().len(), 1);
    assert!(cal.get(agenda.tasks[0].id).await.unwrap().is_none());
}

#[test]
fn the_core_depends_on_no_task_crate() {
    let manifest = include_str!("../Cargo.toml");
    assert!(!manifest.contains("fs-tasks"), "Cargo.toml names fs-tasks");
    let lock = include_str!("../../../Cargo.lock");
    // The workspace lock may name the task crates for the adapter and the app; the core's own entry may not.
    let core = lock
        .split("[[package]]")
        .find(|p| p.contains("name = \"fs-calendar-core\""))
        .expect("the core is in the lock");
    assert!(!core.contains("fs-tasks"), "{core}");
}
