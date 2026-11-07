//! `updateIf` and `Event.revision`: compare-and-set over the wire.

mod common;

use common::{create, event_json, items_of, json, set_id, with_server};
use fs_interfaces::rpc::Problem;

const CONFLICT: &str = "https://freesynergy.net/problems/calendar/conflict";

async fn revision_of(items: &fs_interfaces::events_capnp::items::Client, id: uuid::Uuid) -> u64 {
    let mut get = items.get_request();
    set_id(get.get().init_id(), id);
    let answer = get.send().promise.await.unwrap();
    answer.get().unwrap().get_event().unwrap().get_revision()
}

#[tokio::test(flavor = "current_thread")]
async fn a_write_on_the_revision_read_goes_through_and_a_stale_one_does_not() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        let id = create(&items, "u1", "First", "2026-12-01T10:00:00").await;
        let read = revision_of(&items, id).await;
        assert_ne!(read, 0);
        assert_eq!(
            revision_of(&items, id).await,
            read,
            "reading changes nothing"
        );

        let mut mine = items.update_if_request();
        set_id(mine.get().init_id(), id);
        mine.get().set_revision(read);
        mine.get()
            .set_jscalendar(json("u1", "Mine", "2026-12-01T10:00:00").as_str());
        let answer = mine.send().promise.await.unwrap();
        let event = answer.get().unwrap().get_event().unwrap();
        assert_eq!(event_json(event)["title"], "Mine");
        assert_ne!(event.get_revision(), read, "a write changes the revision");
        assert_eq!(event.get_revision(), revision_of(&items, id).await);

        let mut theirs = items.update_if_request();
        set_id(theirs.get().init_id(), id);
        theirs.get().set_revision(read);
        theirs
            .get()
            .set_jscalendar(json("u1", "Theirs", "2026-12-01T10:00:00").as_str());
        let error = theirs.send().promise.await.err().unwrap();
        let problem = Problem::from_error(&error).unwrap();
        assert_eq!(
            (problem.problem_type.as_str(), problem.status),
            (CONFLICT, 409)
        );
        assert!(
            problem.detail.starts_with("conflict:"),
            "{}",
            problem.detail
        );

        let mut get = items.get_request();
        set_id(get.get().init_id(), id);
        let stored = get.send().promise.await.unwrap();
        assert_eq!(
            event_json(stored.get().unwrap().get_event().unwrap())["title"],
            "Mine"
        );
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn of_two_writers_on_one_revision_exactly_one_wins() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        let id = create(&items, "u2", "First", "2026-12-01T10:00:00").await;
        let read = revision_of(&items, id).await;
        let write = |title: &'static str| {
            let mut call = items.update_if_request();
            set_id(call.get().init_id(), id);
            call.get().set_revision(read);
            call.get()
                .set_jscalendar(json("u2", title, "2026-12-01T10:00:00").as_str());
            call.send().promise
        };
        let (a, b) = tokio::join!(write("A"), write("B"));
        assert_eq!(u8::from(a.is_ok()) + u8::from(b.is_ok()), 1);
    })
    .await;
}
