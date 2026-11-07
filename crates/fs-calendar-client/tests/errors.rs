//! What the client reports it cannot do, and what a failing server looks like.

mod common;

use common::{Served, event, json};
use fs_calendar_core::{Calendar, Error, Event, Response};
use fs_store::{Error as StoreError, ObjectId};

#[tokio::test]
async fn capabilities_name_only_what_the_client_keeps() {
    let served = Served::start().await;
    let caps = served.client().capabilities();
    for kept in [
        "event.create",
        "event.update",
        "event.delete",
        "event.read",
        "event.occurrences",
        "event.import",
        "event.export",
    ] {
        assert!(caps.is_supported(kept), "{kept}");
    }
    for not in [
        "invite.create",
        "invite.answer",
        "invite.read",
        "event.subscribe",
    ] {
        assert!(!caps.is_supported(not), "{not}");
    }
}

#[tokio::test]
async fn the_methods_the_app_never_calls_answer_unsupported() {
    let served = Served::start().await;
    let c = served.client();
    let made = c
        .create(event("u1", "U", "2026-12-01T09:00:00"))
        .await
        .unwrap();
    let id = made.envelope.id;
    assert!(matches!(
        c.invite(id, &[("Ada", "ada@x.org")]).await,
        Err(Error::Unsupported(_))
    ));
    assert!(matches!(
        c.respond(id, "ada@x.org", Response::Accept, None).await,
        Err(Error::Unsupported(_))
    ));
    assert!(matches!(
        c.invitation_log(id).await,
        Err(Error::Unsupported(_))
    ));
    assert!(matches!(
        c.migrate_time_fields().await,
        Err(Error::Unsupported(_))
    ));
}

#[tokio::test]
async fn a_marked_event_in_a_plain_store_is_refused_as_encryption_required_with_its_reason() {
    let served = Served::start_plain().await;
    let c = served.client();
    let mut marked = json("m1", "Private", "2026-12-01T09:00:00");
    marked["freesynergy.net:sensitive"] = serde_json::json!(["dpv:Health"]);
    let refused = c.create(Event::from_value(marked).unwrap()).await;
    assert!(
        matches!(
            refused,
            Err(Error::Store(StoreError::EncryptionRequired(_)))
        ),
        "{refused:?}"
    );
}

#[tokio::test]
async fn a_dense_rule_is_too_expensive() {
    let served = Served::start().await;
    let c = served.client();
    let mut dense = json("d1", "D", "2026-12-01T10:00:00");
    dense["recurrenceRule"] = serde_json::json!({"@type":"RecurrenceRule","frequency":"yearly",
        "byMonth":(1..=12).map(|m| m.to_string()).collect::<Vec<_>>(),"byMonthDay":(1..=31).collect::<Vec<_>>(),
        "byHour":(0..24).collect::<Vec<_>>(),"byMinute":(0..60).collect::<Vec<_>>(),"bySecond":(0..60).collect::<Vec<_>>()});
    let refused = c.create(Event::from_value(dense).unwrap()).await;
    assert!(
        matches!(refused, Err(Error::RuleTooExpensive(_))),
        "{refused:?}"
    );
}

#[tokio::test]
async fn a_server_that_is_gone_is_a_backend_error_that_names_the_server() {
    let mut served = Served::start().await;
    let c = served.client();
    served.stop();
    let gone = c.get(ObjectId::new()).await;
    match gone {
        Err(Error::Store(StoreError::Backend(text))) => {
            assert!(text.contains("calendar"), "{text}")
        }
        other => panic!("{other:?}"),
    }
}

#[tokio::test]
async fn nothing_listening_is_an_error_at_connect() {
    let dir = tempfile::tempdir().unwrap();
    let err = fs_calendar_client::RpcCalendar::connect(&dir.path().join("none.sock"));
    assert!(err.is_err());
}
