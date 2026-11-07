//! What does not fit one message, as a consumer sees it: refused or cut, and the connection stays.

mod common;

use common::{create, event_json, items_of, json, ms, set_id, with_server};
use fs_calendar_core::limits::MAX_EVENT_BYTES;
use fs_interfaces::events_capnp::items;
use fs_interfaces::rpc::{Problem, TOO_LARGE};

const MIB: usize = 1024 * 1024;

fn is(error: &capnp::Error, problem_type: &str, status: u16) {
    let p = Problem::from_error(error).expect("a typed problem");
    assert_eq!((p.problem_type.as_str(), p.status), (problem_type, status));
}

async fn count(items: &items::Client) -> u32 {
    let list = items
        .list_request()
        .send()
        .promise
        .await
        .expect("still connected");
    list.get()
        .unwrap()
        .get_page()
        .unwrap()
        .get_items()
        .unwrap()
        .len()
}

/// The ids exist nowhere: without the check at the door the core would answer "not found".
#[tokio::test(flavor = "current_thread")]
async fn every_text_is_refused_before_it_reaches_the_core() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        let invitations = common::invitations_of(&root).await;
        let huge = "x".repeat(20 * MIB);
        let unknown = fs_store::Envelope::new("Event", "x").id.as_uuid();

        let mut create = items.create_request();
        create
            .get()
            .set_jscalendar(json("u", &huge, "2026-12-01T10:00:00").as_str());
        is(&create.send().promise.await.err().unwrap(), TOO_LARGE, 413);

        let mut update = items.update_request();
        set_id(update.get().init_id(), unknown);
        update.get().set_jscalendar(huge.as_str());
        is(&update.send().promise.await.err().unwrap(), TOO_LARGE, 413);

        let mut by_uid = items.get_by_uid_request();
        by_uid.get().set_uid(huge.as_str());
        is(&by_uid.send().promise.await.err().unwrap(), TOO_LARGE, 413);

        let mut import = items.import_text_request();
        import.get().set_media_type("text/calendar");
        import.get().set_text(huge.as_str());
        is(&import.send().promise.await.err().unwrap(), TOO_LARGE, 413);

        let mut media = items.import_text_request();
        media.get().set_media_type(huge.as_str());
        media.get().set_text("x");
        is(&media.send().promise.await.err().unwrap(), TOO_LARGE, 413);

        for (who, email) in [(true, false), (false, true)] {
            let mut invite = invitations.invite_request();
            set_id(invite.get().init_id(), unknown);
            let mut one = invite.get().init_invitees(1).get(0);
            one.set_name(if who { huge.as_str() } else { "n" });
            one.set_email(if email {
                huge.as_str()
            } else {
                "a@example.org"
            });
            is(&invite.send().promise.await.err().unwrap(), TOO_LARGE, 413);
        }
        let mut respond = invitations.respond_request();
        set_id(respond.get().init_id(), unknown);
        respond.get().set_who("a@example.org");
        respond.get().set_comment(huge.as_str());
        is(&respond.send().promise.await.err().unwrap(), TOO_LARGE, 413);

        assert_eq!(count(&items).await, 0);
    })
    .await;
}

/// An event with a description of `bytes`.
fn big(uid: &str, bytes: usize) -> String {
    let mut v: serde_json::Value =
        serde_json::from_str(&json(uid, "Big", "2026-12-01T10:00:00")).unwrap();
    v["description"] = "x".repeat(bytes).into();
    v.to_string()
}

#[tokio::test(flavor = "current_thread")]
async fn events_at_the_limit_come_back_through_list_and_get_and_one_above_is_refused() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        let mut over = items.create_request();
        over.get()
            .set_jscalendar(big("over", MAX_EVENT_BYTES).as_str());
        is(&over.send().promise.await.err().unwrap(), TOO_LARGE, 413);

        let mut fit = items.create_request();
        let overhead = big("fit", 0).len() + "Big".len();
        fit.get()
            .set_jscalendar(big("fit", MAX_EVENT_BYTES - overhead - 200).as_str());
        let created = fit
            .send()
            .promise
            .await
            .expect("the largest event is accepted");
        let id = fs_interfaces::rpc::read_envelope(
            created
                .get()
                .unwrap()
                .get_event()
                .unwrap()
                .get_envelope()
                .unwrap(),
        )
        .unwrap()
        .id
        .as_uuid();
        create(&items, "small", "Small", "2026-12-02T10:00:00").await;

        let mut seen = 0;
        let mut cursor = vec![];
        loop {
            let mut call = items.list_request();
            call.get().init_cursor().set_opaque(&cursor);
            let answer = call.send().promise.await.expect("the list answers");
            let page = answer.get().unwrap().get_page().unwrap();
            seen += page.get_items().unwrap().len();
            cursor = page.get_next().unwrap().get_opaque().unwrap().to_vec();
            if cursor.is_empty() {
                break;
            }
        }
        assert_eq!(seen, 2);
        let mut get = items.get_request();
        set_id(get.get().init_id(), id);
        let got = get.send().promise.await.expect("get answers");
        let content = event_json(got.get().unwrap().get_event().unwrap());
        assert!(content["description"].as_str().unwrap().len() > MAX_EVENT_BYTES - 1024);
    })
    .await;
}

/// 40 instances of an event of about 1 MiB are more than a message: refused, and the connection lives.
#[tokio::test(flavor = "current_thread")]
async fn an_answer_of_instances_that_does_not_fit_is_refused_not_sent() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        let mut series: serde_json::Value = serde_json::from_str(&big("many", MIB)).unwrap();
        series["recurrenceRule"] =
            serde_json::json!({"@type": "RecurrenceRule", "frequency": "daily"});
        let mut call = items.create_request();
        call.get().set_jscalendar(series.to_string().as_str());
        call.send().promise.await.unwrap();

        let ask = |days: i64| {
            let mut req = items.occurrences_request();
            req.get()
                .init_from()
                .set_unix_millis(ms("2026-12-01T00:00:00Z"));
            req.get()
                .init_to()
                .set_unix_millis(ms("2026-12-01T00:00:00Z") + days * 86_400_000);
            req.send().promise
        };
        let few = ask(5).await.expect("five days fit");
        assert_eq!(few.get().unwrap().get_items().unwrap().len(), 5);
        is(&ask(40).await.err().unwrap(), TOO_LARGE, 413);
        assert_eq!(count(&items).await, 1, "still connected");
    })
    .await;
}
