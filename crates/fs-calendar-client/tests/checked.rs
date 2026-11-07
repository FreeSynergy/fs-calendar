//! The atomic save: the revision the server gives, and `update_checked` against it.

mod common;

use common::{Served, event};
use fs_calendar_client::RevisedCalendar;
use fs_calendar_core::{Calendar, Error, Revision};
use fs_store::ObjectId;

#[tokio::test]
async fn the_revision_read_is_the_one_the_server_checks() {
    let served = Served::start().await;
    let c = served.client();
    let made = c
        .create(event("r1", "First", "2026-12-01T09:00:00"))
        .await
        .unwrap();
    let id = made.envelope.id;
    let read = c.get_revised(id).await.unwrap().unwrap();
    assert_eq!(read.object.content, made.content);
    let saved = c
        .update_checked(
            id,
            &read.revision,
            event("r1", "Second", "2026-12-01T09:00:00"),
        )
        .await
        .unwrap();
    assert_eq!(saved.content["title"], "Second");
    let after = c.get_revised(id).await.unwrap().unwrap();
    assert_ne!(
        after.revision, read.revision,
        "a write changes the revision"
    );
}

#[tokio::test]
async fn a_stale_revision_writes_nothing_and_says_it_was_changed() {
    let served = Served::start().await;
    let c = served.client();
    let made = c
        .create(event("r2", "Mine", "2026-12-01T09:00:00"))
        .await
        .unwrap();
    let id = made.envelope.id;
    let mine = c.get_revised(id).await.unwrap().unwrap().revision;
    // Someone else writes first.
    c.update(id, event("r2", "Theirs", "2026-12-01T09:00:00"))
        .await
        .unwrap();
    let late = c
        .update_checked(id, &mine, event("r2", "Late", "2026-12-01T09:00:00"))
        .await;
    match late {
        Err(e @ Error::Conflict(_)) => assert!(fs_calendar_client::changed_elsewhere(&e), "{e}"),
        other => panic!("{other:?}"),
    }
    let now = Calendar::get(served.repo.as_ref(), id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(now.content["title"], "Theirs", "nothing was written");
}

#[tokio::test]
async fn other_conflicts_are_not_taken_for_a_change() {
    let served = Served::start().await;
    let c = served.client();
    c.create(event("r3", "A", "2026-12-01T09:00:00"))
        .await
        .unwrap();
    let again = c
        .create(event("r3", "B", "2026-12-01T09:00:00"))
        .await
        .unwrap_err();
    assert!(matches!(again, Error::Conflict(_)));
    assert!(!fs_calendar_client::changed_elsewhere(&again));
    let ghost = c.get_revised(ObjectId::new()).await.unwrap();
    assert!(ghost.is_none());
    let gone = c
        .update_checked(
            ObjectId::new(),
            &Revision::from(1),
            event("x", "X", "2026-12-01T09:00:00"),
        )
        .await;
    assert!(matches!(gone, Err(Error::NotFound(_))), "{gone:?}");
}
