//! Failures and the provider's report, as a consumer sees them: the exact problem type, not a status.

mod common;

use std::sync::Arc;

use async_trait::async_trait;
use common::{ME, create, invitations_of, items_of, json, set_id, with_provider, with_server};
use fs_calendar_core::{Calendar, CalendarRepo, Result};
use fs_interfaces::rpc::{INVALID_REQUEST, Problem};
use fs_interfaces::{Capabilities, UNSUPPORTED_PROBLEM_TYPE};
use fs_store::{Envelope, ObjectId, Store, StoredObject, TursoStore};

const INVALID: &str = "https://freesynergy.net/problems/calendar/invalid";
const CONFLICT: &str = "https://freesynergy.net/problems/calendar/conflict";
const NOT_FOUND: &str = "https://freesynergy.net/problems/calendar/not-found";

fn is(error: &capnp::Error, problem_type: &str, status: u16) {
    let p = Problem::from_error(error).expect("a typed problem");
    assert_eq!((p.problem_type.as_str(), p.status), (problem_type, status));
}

#[tokio::test(flavor = "current_thread")]
async fn errors_arrive_typed() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        let invitations = invitations_of(&root).await;
        let id = create(&items, "uid-1", "First", "2026-12-01T10:00:00").await;
        let unknown = Envelope::new("Event", "x").id.as_uuid();

        let mut twice = items.create_request();
        twice.get().set_jscalendar(json("uid-1", "Again", "2026-12-01T10:00:00").as_str());
        is(&twice.send().promise.await.err().unwrap(), CONFLICT, 409);

        let mut broken = items.create_request();
        broken.get().set_jscalendar("{ not json");
        is(&broken.send().promise.await.err().unwrap(), INVALID, 422);

        let mut zone = items.create_request();
        zone.get().set_jscalendar(
            r#"{"@type":"Event","version":"2.0","uid":"z","title":"Z","start":"2026-12-01T10:00:00","timeZone":"Mars/Base"}"#,
        );
        is(&zone.send().promise.await.err().unwrap(), INVALID, 422);

        let mut other_uid = items.update_request();
        set_id(other_uid.get().init_id(), id);
        other_uid.get().set_jscalendar(json("other", "x", "2026-12-01T10:00:00").as_str());
        is(&other_uid.send().promise.await.err().unwrap(), CONFLICT, 409);

        let mut delete = items.delete_request();
        set_id(delete.get().init_id(), unknown);
        is(&delete.send().promise.await.err().unwrap(), NOT_FOUND, 404);

        let mut update = items.update_request();
        set_id(update.get().init_id(), unknown);
        update.get().set_jscalendar(json("uid-1", "x", "2026-12-01T10:00:00").as_str());
        is(&update.send().promise.await.err().unwrap(), NOT_FOUND, 404);

        let mut invite = invitations.invite_request();
        set_id(invite.get().init_id(), unknown);
        is(&invite.send().promise.await.err().unwrap(), NOT_FOUND, 404);

        let mut no_id = items.get_request();
        no_id.get().init_id();
        is(&no_id.send().promise.await.err().unwrap(), INVALID_REQUEST, 422);

        let mut cursor = items.list_request();
        cursor.get().init_cursor().set_opaque(b"bad");
        is(&cursor.send().promise.await.err().unwrap(), INVALID_REQUEST, 422);

        let mut mail = invitations.invite_request();
        set_id(mail.get().init_id(), id);
        let mut one = mail.get().init_invitees(1).get(0);
        one.set_name("n");
        one.set_email("");
        is(&mail.send().promise.await.err().unwrap(), INVALID, 422);
    })
    .await;
}

/// A provider that offers the reads and nothing else: every other method answers `Unsupported`.
struct ReadOnly;

#[async_trait]
impl Calendar for ReadOnly {
    fn capabilities(&self) -> Capabilities {
        let mut caps = Capabilities::new("read-only");
        caps.supported("event.read");
        caps.unsupported("event.create", "this provider only reads");
        caps
    }
    async fn get(&self, _: ObjectId) -> Result<Option<StoredObject>> {
        Ok(None)
    }
    async fn get_event(&self, _: ObjectId) -> Result<Option<fs_calendar_core::Event>> {
        Ok(None)
    }
    async fn get_by_uid(&self, _: &str) -> Result<Option<StoredObject>> {
        Ok(None)
    }
    async fn list_events(&self) -> Result<Vec<StoredObject>> {
        Ok(vec![])
    }
    async fn occurrences(
        &self,
        _: chrono::DateTime<chrono::Utc>,
        _: chrono::DateTime<chrono::Utc>,
    ) -> Result<Vec<fs_calendar_core::Instance>> {
        Ok(vec![])
    }
}

#[tokio::test(flavor = "current_thread")]
async fn every_method_a_provider_does_not_offer_answers_unsupported_with_its_capability() {
    let store: Arc<dyn Store> = Arc::new(TursoStore::open_in_memory().await.unwrap());
    let feed = Arc::new(CalendarRepo::new(store, ME));
    with_provider(Arc::new(ReadOnly), feed, |root| async move {
        let items = items_of(&root).await;
        let invitations = invitations_of(&root).await;
        let id = Envelope::new("Event", "x").id.as_uuid();
        let expect = |error: capnp::Error, capability: &str| {
            let p = Problem::from_error(&error).expect("a typed problem");
            assert_eq!(
                (p.problem_type.as_str(), p.status),
                (UNSUPPORTED_PROBLEM_TYPE, 501),
                "{capability}"
            );
            assert!(p.detail.contains(capability), "{capability}: {}", p.detail);
        };
        let mut create = items.create_request();
        create
            .get()
            .set_jscalendar(json("u", "t", "2026-12-01T10:00:00").as_str());
        expect(create.send().promise.await.err().unwrap(), "event.create");
        let mut update = items.update_request();
        set_id(update.get().init_id(), id);
        update
            .get()
            .set_jscalendar(json("u", "t", "2026-12-01T10:00:00").as_str());
        expect(update.send().promise.await.err().unwrap(), "event.update");
        let mut delete = items.delete_request();
        set_id(delete.get().init_id(), id);
        expect(delete.send().promise.await.err().unwrap(), "event.delete");
        let mut import = items.import_text_request();
        import.get().set_media_type("text/calendar");
        import.get().set_text("x");
        expect(import.send().promise.await.err().unwrap(), "event.import");
        let mut export = items.export_text_request();
        export.get().set_media_type("text/calendar");
        expect(export.send().promise.await.err().unwrap(), "event.export");
        let mut invite = invitations.invite_request();
        set_id(invite.get().init_id(), id);
        expect(invite.send().promise.await.err().unwrap(), "invite.create");
        let mut respond = invitations.respond_request();
        set_id(respond.get().init_id(), id);
        respond.get().set_who("a@example.org");
        expect(respond.send().promise.await.err().unwrap(), "invite.answer");
        let mut history = invitations.history_request();
        set_id(history.get().init_id(), id);
        expect(history.send().promise.await.err().unwrap(), "invite.read");
    })
    .await;
}
