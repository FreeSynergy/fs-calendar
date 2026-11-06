//! Task queries: due dates, ranges and ordering in the store.

mod common;

use common::{repo, t};
use fs_calendar_core::*;
use serde_json::json;

#[tokio::test]
async fn tasks_by_due_date() {
    let r = repo().await;
    let mut a = Task::new("Write the minutes");
    a.due = Some("2026-11-12T17:00:00".into());
    a.time_zone = Some("Europe/Berlin".into());
    a.set_vendor("freesynergy.net:taskStatus", json!("waiting-for-input"));
    let b = Task::new("Someday");
    let oa = r.create(a).await.unwrap();
    r.create(b).await.unwrap();
    assert_eq!(oa.envelope.object_type, "Task");
    assert_eq!(oa.index["due_utc"], ["2026-11-12T16:00:00Z"]);
    assert_eq!(oa.index["progress"], ["needs-action"]);
    let due = r
        .tasks_due(t("2026-11-01T00:00:00Z"), t("2026-12-01T00:00:00Z"))
        .await
        .unwrap();
    assert_eq!(due.len(), 1);
    assert_eq!(r.list_tasks().await.unwrap().len(), 2);
    assert_eq!(
        r.list_tasks().await.unwrap()[0].envelope.name,
        "Write the minutes",
        "undated last"
    );
    let mut task = r.get_task(oa.envelope.id).await.unwrap().unwrap();
    assert_eq!(
        task.vendor("freesynergy.net:taskStatus"),
        Some(&json!("waiting-for-input"))
    );
    task.progress = Some("completed".into());
    task.percent_complete = Some(100);
    r.update(oa.envelope.id, task).await.unwrap();
    assert!(r.get_event(oa.envelope.id).await.unwrap().is_none());
    assert!(
        r.occurrences(t("2020-01-01T00:00:00Z"), t("2040-01-01T00:00:00Z"))
            .await
            .unwrap()
            .is_empty(),
        "tasks are not events"
    );
}

#[tokio::test]
async fn task_ranges_and_ordering_in_the_store() {
    let r = repo().await;
    for (title, due) in [
        ("c", Some("2026-11-12T17:00:00")),
        ("a", Some("2026-11-10T17:00:00")),
        ("none", None),
        ("d", Some("2026-12-31T17:00:00")),
    ] {
        let mut task = Task::new(title);
        task.due = due.map(Into::into);
        task.time_zone = due.map(|_| "Etc/UTC".to_string());
        r.create(task).await.unwrap();
    }
    let order: Vec<_> = r
        .list_tasks()
        .await
        .unwrap()
        .into_iter()
        .map(|o| o.envelope.name)
        .collect();
    assert_eq!(order, ["a", "c", "d", "none"]);
    let due: Vec<_> = r
        .tasks_due(t("2026-11-11T00:00:00Z"), t("2026-12-31T17:00:00Z"))
        .await
        .unwrap()
        .into_iter()
        .map(|o| o.envelope.name)
        .collect();
    assert_eq!(due, ["c"], "[from, to)");
}
