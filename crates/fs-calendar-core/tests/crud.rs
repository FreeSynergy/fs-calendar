//! Create / read / update / delete, envelope, index fields and `sequence`.

mod common;

use common::{ME, setup};
use fs_calendar_core::*;
use serde_json::json;

#[tokio::test]
async fn crud_envelope_index_and_sequence() {
    let (r, store) = setup().await;
    let mut e = Event::new("Jour fixe", "2026-11-03T10:00:00", "Europe/Berlin", "PT30M");
    e.recurrence_rule =
        Some(serde_json::from_value(json!({"frequency":"weekly","count":3})).unwrap());
    e.add_invitee("Ben", "Ben@Example.org");
    let o = r.create(e.clone()).await.unwrap();
    assert_eq!(o.envelope.object_type, "Event");
    assert_eq!(o.envelope.name, "Jour fixe");
    assert_eq!(o.envelope.context.as_deref(), Some("calendar"));
    assert_eq!(o.content_type, "application/jscalendar+json");
    assert_eq!(o.index["start_utc"], ["2026-11-03T09:00:00Z"]);
    assert_eq!(o.index["end_utc"], ["2026-11-17T09:30:00Z"]);
    assert_eq!(o.index["recurring"], ["true"]);
    assert_eq!(o.index["participant_email"], ["ben@example.org"]);
    assert!(o.envelope.iri.as_deref().unwrap().contains("/events/"));
    assert!(matches!(r.create(e.clone()).await, Err(Error::Conflict(_))));
    let id = o.envelope.id;
    let mut ev = r.get_event(id).await.unwrap().unwrap();
    assert_eq!(ev.sequence, None);
    ev.title = Some("Jour fixe (neu)".into());
    let o2 = r.update(id, ev.clone()).await.unwrap();
    assert_eq!(o2.envelope.name, "Jour fixe (neu)");
    assert_eq!(
        r.get_event(id).await.unwrap().unwrap().sequence,
        None,
        "title is not scheduling relevant"
    );
    ev.start = Some("2026-11-03T11:00:00".into());
    r.update(id, ev.clone()).await.unwrap();
    let after = r.get_event(id).await.unwrap().unwrap();
    assert_eq!(after.sequence, Some(1));
    assert_eq!(r.get(id).await.unwrap().unwrap().index["sequence"], ["1"]);
    let mut other = ev.clone();
    other.uid = "other".into();
    assert!(matches!(r.update(id, other).await, Err(Error::Conflict(_))));
    r.delete(id).await.unwrap();
    assert!(r.get(id).await.unwrap().is_none());
    store.undo(ME, 1).await.unwrap();
    assert!(r.get(id).await.unwrap().is_some());
    // invalid input is refused
    let mut bad = Event::new("x", "2026-11-03T11:00:00", "Mars/Base", "PT1H");
    bad.uid = "bad".into();
    assert!(matches!(r.create(bad).await, Err(Error::Invalid(_))));
}
