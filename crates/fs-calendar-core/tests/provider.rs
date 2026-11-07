//! The provider rule (ADR 0045): every provider has every method; what it does not offer answers
//! an explicit "not possible", and its report says the same.

mod common;

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use common::{repo, t};
use fs_calendar_core::{Calendar, Capabilities, Error, Event, Instance, Response, Result};
use fs_objects::{ObjectId, StoredObject};

/// A read-only provider, for example a calendar feed it cannot write to.
struct ReadOnly;

#[async_trait]
impl Calendar for ReadOnly {
    fn capabilities(&self) -> Capabilities {
        let mut c = Capabilities::new("read-only");
        c.supported("event.read");
        c.unsupported("event.create", "this feed cannot be written");
        c
    }
    async fn get(&self, _: ObjectId) -> Result<Option<StoredObject>> {
        Ok(None)
    }
    async fn get_event(&self, _: ObjectId) -> Result<Option<Event>> {
        Ok(None)
    }
    async fn get_by_uid(&self, _: &str) -> Result<Option<StoredObject>> {
        Ok(None)
    }
    async fn list_events(&self) -> Result<Vec<StoredObject>> {
        Ok(vec![])
    }
    async fn occurrences(&self, _: DateTime<Utc>, _: DateTime<Utc>) -> Result<Vec<Instance>> {
        Ok(vec![])
    }
}

fn capability(r: Result<()>) -> String {
    match r {
        Err(Error::Unsupported(u)) => u.capability().to_string(),
        other => panic!("expected an explicit not possible, got {other:?}"),
    }
}

#[tokio::test]
async fn a_missing_method_says_not_possible() {
    let p: Arc<dyn Calendar> = Arc::new(ReadOnly);
    let (a, b) = (t("2026-11-01T00:00:00Z"), t("2026-12-01T00:00:00Z"));
    assert!(p.upcoming(a, 30).await.unwrap().is_empty());
    let e = Event::new("x", "2026-11-24T10:00:00", "Europe/Berlin", "PT1H");
    let id = ObjectId::new();
    assert_eq!(
        capability(p.create(e.clone()).await.map(|_| ())),
        "event.create"
    );
    assert_eq!(
        capability(p.update(id, e).await.map(|_| ())),
        "event.update"
    );
    assert_eq!(capability(p.delete(id).await), "event.delete");
    assert_eq!(
        capability(p.invite(id, &[]).await.map(|_| ())),
        "invite.create"
    );
    let answer = p.respond(id, "a@b.c", Response::Accept, None).await;
    assert_eq!(capability(answer.map(|_| ())), "invite.answer");
    assert_eq!(
        capability(p.invitation_log(id).await.map(|_| ())),
        "invite.read"
    );
    assert_eq!(
        capability(p.agenda(a, b).await.map(|_| ())),
        "event.occurrences"
    );
    assert_eq!(
        capability(p.import_text("text/calendar", "").await.map(|_| ())),
        "event.import"
    );
    assert_eq!(
        capability(p.export_text(&[], "text/calendar").await.map(|_| ())),
        "event.export"
    );
    assert!(!p.capabilities().is_supported("event.create"));
}

#[tokio::test]
async fn the_store_backed_provider_reports_what_it_can() {
    let p: Arc<dyn Calendar> = Arc::new(repo().await);
    let caps = p.capabilities();
    for id in [
        "event.create",
        "event.occurrences",
        "event.import",
        "invite.answer",
        "event.subscribe",
    ] {
        assert!(caps.is_supported(id), "{id}");
    }
}
