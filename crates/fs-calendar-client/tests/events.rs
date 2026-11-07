//! The events the app reads and writes, through the real server: values compared, not just "it ran".

mod common;

use common::{Served, event, json};
use fs_calendar_core::{Calendar, Error};
use fs_store::ObjectId;

#[tokio::test]
async fn an_event_made_through_the_client_reads_back_equal_in_every_way() {
    let served = Served::start().await;
    let c = served.client();
    let made = c
        .create(event("a1", "Standup", "2026-12-01T09:00:00"))
        .await
        .unwrap();
    assert_eq!(made.envelope.name, "Standup");
    assert_eq!(made.content["start"], "2026-12-01T09:00:00");
    let by_id = c.get(made.envelope.id).await.unwrap().unwrap();
    let by_uid = c.get_by_uid("a1").await.unwrap().unwrap();
    let typed = c.get_event(made.envelope.id).await.unwrap().unwrap();
    assert_eq!(by_id.content, made.content);
    assert_eq!(by_uid.envelope.id, made.envelope.id);
    assert_eq!(typed.title.as_deref(), Some("Standup"));
    // The server's own store holds the very same thing.
    let behind = Calendar::get(served.repo.as_ref(), made.envelope.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(behind.content, by_id.content);
    assert_eq!(behind.envelope.context, by_id.envelope.context);
}

#[tokio::test]
async fn an_update_and_a_delete_reach_the_store() {
    let served = Served::start().await;
    let c = served.client();
    let made = c
        .create(event("a2", "Old", "2026-12-01T09:00:00"))
        .await
        .unwrap();
    let id = made.envelope.id;
    let changed = event("a2", "New", "2026-12-01T10:00:00");
    let updated = c.update(id, changed).await.unwrap();
    assert_eq!(updated.content["title"], "New");
    assert_eq!(updated.content["start"], "2026-12-01T10:00:00");
    c.delete(id).await.unwrap();
    assert!(c.get(id).await.unwrap().is_none());
    assert!(
        Calendar::get(served.repo.as_ref(), id)
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn what_is_missing_is_none_and_a_write_to_it_is_not_found_with_its_id() {
    let served = Served::start().await;
    let c = served.client();
    let ghost = ObjectId::new();
    assert!(c.get(ghost).await.unwrap().is_none());
    assert!(c.get_by_uid("nobody").await.unwrap().is_none());
    match c
        .update(ghost, event("g", "G", "2026-12-01T09:00:00"))
        .await
    {
        Err(Error::NotFound(id)) => assert_eq!(id, ghost),
        other => panic!("{other:?}"),
    }
    assert!(matches!(c.delete(ghost).await, Err(Error::NotFound(id)) if id == ghost));
}

#[tokio::test]
async fn a_taken_uid_is_a_conflict_and_a_bad_event_names_its_issues() {
    let served = Served::start().await;
    let c = served.client();
    c.create(event("dup", "One", "2026-12-01T09:00:00"))
        .await
        .unwrap();
    let again = c.create(event("dup", "Two", "2026-12-02T09:00:00")).await;
    assert!(matches!(again, Err(Error::Conflict(_))), "{again:?}");
    // The core refuses this one before the server sees it, so make it past the typed event.
    let mut bad = json("bad", "Bad", "2026-12-01T09:00:00");
    bad["duration"] = "PT1H".into();
    bad["status"] = "nonsense".into();
    let bad = fs_calendar_core::Event::from_value(bad).unwrap();
    match c.create(bad).await {
        Err(Error::Invalid(issues)) => assert!(
            issues.iter().any(|i| i.path.contains("status")),
            "{issues:?}"
        ),
        other => panic!("{other:?}"),
    }
}
