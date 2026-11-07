//! Compare-and-set on `update_checked`, against the trait.

mod common;

use std::sync::Arc;

use common::repo;
use fs_calendar_core::*;

fn event() -> Event {
    Event::new("Jour fixe", "2026-12-01T10:00:00", "Europe/Berlin", "PT1H")
}

#[tokio::test]
async fn a_write_on_a_stale_revision_is_refused_and_writes_nothing() {
    let calendar: Arc<dyn Calendar> = Arc::new(repo().await);
    let made = calendar.create(event()).await.unwrap();
    let id = made.envelope.id;
    let read = Revision::of(&made);
    let mut mine = calendar.get_event(id).await.unwrap().unwrap();
    mine.title = Some("Mine".into());
    let after_mine = calendar.update_checked(id, &read, mine).await.unwrap();
    assert_ne!(
        Revision::of(&after_mine),
        read,
        "a write changes the revision"
    );

    let mut theirs = calendar.get_event(id).await.unwrap().unwrap();
    theirs.title = Some("Theirs".into());
    let got = calendar.update_checked(id, &read, theirs).await;
    assert!(matches!(got, Err(Error::Conflict(_))), "{got:?}");
    let stored = calendar.get(id).await.unwrap().unwrap();
    assert_eq!(stored.envelope.name, "Mine");
    assert_eq!(Revision::of(&stored), Revision::of(&after_mine));
}

#[tokio::test]
async fn of_two_writers_on_one_revision_exactly_one_wins() {
    let calendar: Arc<dyn Calendar> = Arc::new(repo().await);
    let made = calendar.create(event()).await.unwrap();
    let id = made.envelope.id;
    let read = Revision::of(&made);
    let writer = |title: &'static str| {
        let calendar = calendar.clone();
        let read = read.clone();
        async move {
            let mut e = calendar.get_event(id).await.unwrap().unwrap();
            e.title = Some(title.into());
            calendar.update_checked(id, &read, e).await
        }
    };
    let (a, b) = tokio::join!(writer("A"), writer("B"));
    assert_eq!(a.is_ok() as u8 + b.is_ok() as u8, 1, "{a:?} / {b:?}");
    let loser = if a.is_err() { a } else { b };
    assert!(matches!(loser, Err(Error::Conflict(_))));
}

#[tokio::test]
async fn a_missing_event_is_not_found_not_a_conflict() {
    let calendar: Arc<dyn Calendar> = Arc::new(repo().await);
    let made = calendar.create(event()).await.unwrap();
    let gone = fs_store::ObjectId::new();
    let got = calendar
        .update_checked(gone, &Revision::of(&made), event())
        .await;
    assert!(matches!(got, Err(Error::NotFound(_))), "{got:?}");
}
