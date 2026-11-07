//! A sensitive marking in the content reaches the envelope, so the store's rule (ADR 0029) applies.

mod common;

use std::sync::Arc;

use common::ME;
use fs_calendar_core::*;
use fs_store::{OpenOptions, Store, TursoStore};
use serde_json::json;

/// The in-memory store accepts marked objects (as tests may).
async fn tolerant() -> CalendarRepo {
    let store: Arc<dyn Store> = Arc::new(TursoStore::open_in_memory().await.unwrap());
    CalendarRepo::new(store, ME)
}

/// A store that is not encrypted and does not opt out: the rule of ADR 0029 applies.
async fn strict() -> CalendarRepo {
    let store = TursoStore::open_with(":memory:", OpenOptions::default());
    CalendarRepo::new(Arc::new(store.await.unwrap()), ME)
}

fn event(title: &str) -> Event {
    Event::new(title, "2026-12-01T10:00:00", "Europe/Berlin", "PT1H")
}

#[tokio::test]
async fn a_marked_event_is_refused_by_a_store_that_is_not_encrypted() {
    let r = strict().await;
    let mut e = event("Doctor");
    e.set_vendor("freesynergy.net:sensitive", json!(["dpv:HealthRecord"]));
    let got = r.create(e).await;
    assert!(matches!(got, Err(Error::Store(_))), "{got:?}");
    assert!(r.list_events().await.unwrap().is_empty());
}

#[tokio::test]
async fn the_categories_are_on_the_envelope_wherever_they_are_in_the_event() {
    let r = tolerant().await;
    let mut e = event("Doctor");
    e.set_vendor("freesynergy.net:sensitive", json!(["dpv:HealthRecord"]));
    let mut p = Participant::person("Ben", "ben@example.org", "attendee");
    p.extra.insert(
        "freesynergy.net:sensitive".into(),
        json!(["dpv:Religion", "dpv:HealthRecord"]),
    );
    e.participants.insert("p1".into(), p);
    let made = r.create(e).await.unwrap();
    assert_eq!(
        made.envelope.sensitive,
        ["dpv:HealthRecord", "dpv:Religion"],
        "sorted, once each"
    );
}

#[tokio::test]
async fn a_marking_without_a_readable_category_is_marked_as_unclear() {
    let r = tolerant().await;
    let mut e = event("Unclear");
    e.set_vendor("freesynergy.net:sensitive", json!(true));
    let made = r.create(e).await.unwrap();
    assert_eq!(made.envelope.sensitive, [UNCLEAR_SENSITIVE]);
}

#[tokio::test]
async fn an_unmarked_event_has_no_categories_and_privacy_alone_marks_nothing() {
    let r = tolerant().await;
    let mut e = event("Private");
    e.privacy = Some("private".into());
    let made = r.create(e).await.unwrap();
    assert!(made.envelope.sensitive.is_empty());
}

#[tokio::test]
async fn an_update_never_takes_a_marking_away() {
    let r = tolerant().await;
    let mut e = event("Doctor");
    e.set_vendor("freesynergy.net:sensitive", json!(["dpv:HealthRecord"]));
    let made = r.create(e).await.unwrap();
    let id = made.envelope.id;
    let mut stripped = r.get_event(id).await.unwrap().unwrap();
    stripped.extra.remove("freesynergy.net:sensitive");
    let after = r.update(id, stripped).await.unwrap();
    assert_eq!(after.envelope.sensitive, ["dpv:HealthRecord"]);
}

#[tokio::test]
async fn an_import_marks_the_envelope_too() {
    let r = tolerant().await;
    let json = r#"{"@type":"Event","version":"2.0","uid":"m1","title":"M","start":"2026-12-01T10:00:00","freesynergy.net:sensitive":["dpv:Religion"]}"#;
    let report = r
        .import_text("application/jscalendar+json", json)
        .await
        .unwrap();
    let stored = r.get(report.created[0]).await.unwrap().unwrap();
    assert_eq!(stored.envelope.sensitive, ["dpv:Religion"]);
}
