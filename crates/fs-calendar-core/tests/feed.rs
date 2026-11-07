//! `event.subscribe`: every change reaches a listener until its subscription is dropped.

mod common;

use std::sync::{Arc, Mutex};

use common::repo;
use fs_calendar_core::*;

#[derive(Default)]
struct Seen(Mutex<Vec<CalendarChange>>);

impl CalendarListener for Seen {
    fn changed(&self, change: &CalendarChange) {
        self.0.lock().unwrap().push(*change);
    }
}

fn event(uid: &str) -> Event {
    let mut e = Event::new("Watched", "2026-12-01T10:00:00", "Europe/Berlin", "PT1H");
    e.uid = uid.into();
    e
}

#[tokio::test]
async fn every_kind_of_write_is_told_and_a_dropped_subscription_hears_nothing() {
    let repo = repo().await;
    let feed: &dyn CalendarFeed = &repo;
    let seen = Arc::new(Seen::default());
    let subscription = feed.subscribe(seen.clone());

    let made = repo.create(event("a")).await.unwrap();
    let id = made.envelope.id;
    let mut changed = repo.get_event(id).await.unwrap().unwrap();
    changed.title = Some("Changed".into());
    repo.update(id, changed).await.unwrap();
    repo.invite(id, &[("Ben", "ben@example.org")])
        .await
        .unwrap();
    repo.respond(id, "ben@example.org", Response::Accept, None)
        .await
        .unwrap();
    repo.delete(id).await.unwrap();
    let ics = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:x\r\nBEGIN:VEVENT\r\nUID:imp\r\nDTSTAMP:20261101T000000Z\r\nDTSTART:20261201T090000Z\r\nSUMMARY:I\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
    let report = repo.import_text("text/calendar", ics).await.unwrap();
    let imported = report.created[0];

    assert_eq!(
        *seen.0.lock().unwrap(),
        [
            CalendarChange::Created(id),
            CalendarChange::Updated(id),
            CalendarChange::Updated(id),
            CalendarChange::Updated(id),
            CalendarChange::Deleted(id),
            CalendarChange::Created(imported),
        ]
    );
    drop(subscription);
    repo.create(event("b")).await.unwrap();
    assert_eq!(seen.0.lock().unwrap().len(), 6, "told after the drop");
}

#[tokio::test]
async fn a_refused_write_tells_nobody() {
    let repo = repo().await;
    let seen = Arc::new(Seen::default());
    let _subscription = CalendarFeed::subscribe(&repo, seen.clone());
    repo.create(event("a")).await.unwrap();
    assert!(repo.create(event("a")).await.is_err());
    assert!(repo.delete(fs_objects::ObjectId::new()).await.is_err());
    assert_eq!(seen.0.lock().unwrap().len(), 1);
}
