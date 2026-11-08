//! Invitations as log activities, atomic replies, legacy log entries (ADR 0020).

mod common;

use std::sync::Arc;

use common::{ME, setup};
use fs_calendar_core::*;
use fs_objects::{ActivityFilter, ActivityKind, Store, TursoStore};
use serde_json::json;

#[allow(clippy::too_many_lines)] // one linear walk through an invitation round, 3 lines over
#[tokio::test]
async fn invitations_are_log_activities() {
    let (r, store) = setup().await;
    let mut e = Event::new("Planning", "2026-11-24T10:00:00", "Europe/Berlin", "PT1H");
    e.add_invitee("Me", "me@example.org");
    let o = r.create(e).await.unwrap();
    let id = o.envelope.id;
    let o = r
        .invite(
            id,
            &[("Ben", "ben@example.org"), ("Me again", "ME@example.org")],
        )
        .await
        .unwrap();
    let ev = Event::from_value(o.content.clone()).unwrap();
    assert_eq!(ev.participants.len(), 2, "existing address skipped");
    assert_eq!(o.index["participant_email"].len(), 2);

    r.respond(
        id,
        "ben@example.org",
        Response::TentativeAccept,
        Some("maybe"),
    )
    .await
    .unwrap();
    let ev = r.get_event(id).await.unwrap().unwrap();
    let key = ev
        .participant_by_email("BEN@example.org")
        .unwrap()
        .to_string();
    assert_eq!(
        ev.participants[&key].participation_status.as_deref(),
        Some("tentative")
    );
    assert_eq!(
        ev.participants[&key].participation_comment.as_deref(),
        Some("maybe")
    );
    r.respond(id, &key, Response::Accept, None).await.unwrap();
    assert_eq!(
        r.get_event(id).await.unwrap().unwrap().participants[&key]
            .participation_status
            .as_deref(),
        Some("accepted")
    );
    r.respond(id, "me@example.org", Response::Reject, Some("clash"))
        .await
        .unwrap();
    let ev = r.get_event(id).await.unwrap().unwrap();
    assert_eq!(
        ev.participants["p1"].participation_status.as_deref(),
        Some("declined")
    );
    assert!(matches!(
        r.respond(id, "nobody@example.org", Response::Accept, None)
            .await,
        Err(Error::Conflict(_))
    ));

    let log: Vec<_> = r
        .invitation_log(id)
        .await
        .unwrap()
        .into_iter()
        .map(|e| (e.activity, e.email.unwrap_or_default()))
        .collect();
    assert_eq!(
        log,
        [
            ("Invite".to_string(), "ben@example.org".to_string()),
            ("TentativeAccept".into(), "ben@example.org".into()),
            ("Accept".into(), "ben@example.org".into()),
            ("Reject".into(), "me@example.org".into()),
        ]
    );
    // written with the real AS2 kinds (fs-objects 0.2), no payload workaround
    let kinds: Vec<_> = store
        .activities(&ActivityFilter {
            object: Some(id),
            ..Default::default()
        })
        .await
        .unwrap()
        .into_iter()
        .map(|a| a.kind)
        .collect();
    for k in [
        ActivityKind::Invite,
        ActivityKind::TentativeAccept,
        ActivityKind::Accept,
        ActivityKind::Reject,
    ] {
        assert!(kinds.contains(&k), "{k:?}");
    }
    assert!(!kinds.contains(&ActivityKind::Offer));
    // a cancelled event cannot be answered
    let mut ev = r.get_event(id).await.unwrap().unwrap();
    ev.status = Some("cancelled".into());
    r.update(id, ev).await.unwrap();
    assert!(matches!(
        r.respond(id, "ben@example.org", Response::Accept, None)
            .await,
        Err(Error::Conflict(_))
    ));
    assert!(
        r.get_event(id).await.unwrap().unwrap().sequence.unwrap() >= 1,
        "cancelling bumps the sequence"
    );
}

#[tokio::test]
async fn respond_is_atomic_with_its_log_entry() {
    let store = Arc::new(TursoStore::open_in_memory().await.unwrap());
    let r = CalendarRepo::new(store.clone() as Arc<dyn Store>, ME);
    let mut e = Event::new("Planning", "2026-11-24T10:00:00", "Europe/Berlin", "PT1H");
    e.add_invitee("Ben", "ben@example.org");
    let id = r.create(e).await.unwrap().envelope.id;
    let log_len = || async {
        store
            .activities(&ActivityFilter {
                object: Some(id),
                ..Default::default()
            })
            .await
            .unwrap()
            .len()
    };
    let before = r.get(id).await.unwrap().unwrap();
    let n = log_len().await;
    // crash between the Update and the Accept entry: neither survives
    store.inject_fault_before_extra(true);
    assert!(
        r.respond(id, "ben@example.org", Response::Accept, None)
            .await
            .is_err()
    );
    assert_eq!(
        r.get(id).await.unwrap().unwrap(),
        before,
        "status unchanged"
    );
    assert_eq!(log_len().await, n);
    assert_eq!(
        r.invitation_log(id).await.unwrap(),
        [] as [fs_calendar_core::InvitationEntry; 0]
    );
    // same for invite
    store.inject_fault_before_extra(true);
    assert!(r.invite(id, &[("Zed", "zed@example.org")]).await.is_err());
    assert_eq!(r.get(id).await.unwrap().unwrap(), before);
    // and it works afterwards: one Update + one Accept
    r.respond(id, "ben@example.org", Response::Accept, None)
        .await
        .unwrap();
    assert_eq!(log_len().await, n + 2);
    assert_eq!(r.invitation_log(id).await.unwrap().len(), 1);
}

#[tokio::test]
async fn legacy_invitation_entries_still_read() {
    let (r, store) = setup().await;
    let mut e = Event::new("Old", "2026-11-24T10:00:00", "Europe/Berlin", "PT1H");
    e.add_invitee("Ben", "ben@example.org");
    let id = r.create(e).await.unwrap().envelope.id;
    store
        .log(
            ActivityKind::Offer,
            id,
            ME,
            json!({"type": "Invite", "participant": "p1", "email": "ben@example.org"}),
        )
        .await
        .unwrap();
    store
        .log(
            ActivityKind::Accept,
            id,
            ME,
            json!({"type": "TentativeAccept", "participant": "p1", "email": "ben@example.org"}),
        )
        .await
        .unwrap();
    let names: Vec<_> = r
        .invitation_log(id)
        .await
        .unwrap()
        .into_iter()
        .map(|e| e.activity)
        .collect();
    assert_eq!(names, ["Invite", "TentativeAccept"]);
}
