//! `Items`: create, update, delete, get, getByUid, list, over a real socket, comparing values.

mod common;

use std::collections::BTreeSet;

use common::{create, event_json, items_of, json, ms, set_id, with_server};
use fs_interfaces::rpc::read_envelope;

#[tokio::test(flavor = "current_thread")]
async fn an_event_goes_in_and_comes_out_the_same_everywhere() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        let mut req = items.create_request();
        let sent = json(
            "urn:uuid:0192f6a0-7c3e-7b1e-8a3d-5f6c7d8e9f01",
            "Jour fixe",
            "2026-12-01T10:00:00",
        );
        req.get().set_jscalendar(sent.as_str());
        let answer = req.send().promise.await.unwrap();
        let event = answer.get().unwrap().get_event().unwrap();
        let env = read_envelope(event.get_envelope().unwrap()).unwrap();
        let content = event_json(event);
        assert_eq!(content["title"], "Jour fixe");
        assert_eq!(content["start"], "2026-12-01T10:00:00");
        assert_eq!(content["timeZone"], "Europe/Berlin");
        assert_eq!(
            env.start_time.unwrap().timestamp_millis(),
            ms("2026-12-01T09:00:00Z")
        );
        assert_eq!(
            env.end_time.unwrap().timestamp_millis(),
            ms("2026-12-01T10:00:00Z")
        );
        assert_eq!(env.id.to_string(), "0192f6a0-7c3e-7b1e-8a3d-5f6c7d8e9f01");
        assert!(env.iri.unwrap().ends_with(&format!("/events/{}", env.id)));
        let id = env.id.as_uuid();

        let mut get = items.get_request();
        set_id(get.get().init_id(), id);
        let got = get.send().promise.await.unwrap();
        let got = got.get().unwrap();
        assert!(got.get_found());
        assert_eq!(event_json(got.get_event().unwrap()), content);

        // the same uid in another spelling is the same event
        for spelling in [
            "0192f6a0-7c3e-7b1e-8a3d-5f6c7d8e9f01",
            "URN:UUID:0192F6A0-7C3E-7B1E-8A3D-5F6C7D8E9F01",
        ] {
            let mut by = items.get_by_uid_request();
            by.get().set_uid(spelling);
            let found = by.send().promise.await.unwrap();
            let found = found.get().unwrap();
            assert!(found.get_found(), "{spelling}");
            assert_eq!(event_json(found.get_event().unwrap()), content);
        }
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn update_replaces_and_a_scheduling_change_raises_the_sequence_delete_removes() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        let id = create(&items, "u1", "First", "2026-12-01T10:00:00").await;
        let mut update = items.update_request();
        set_id(update.get().init_id(), id);
        update
            .get()
            .set_jscalendar(json("u1", "Moved", "2026-12-01T11:00:00").as_str());
        let answer = update.send().promise.await.unwrap();
        let event = answer.get().unwrap().get_event().unwrap();
        let content = event_json(event);
        assert_eq!(content["title"], "Moved");
        assert_eq!(content["sequence"], 1);
        let env = read_envelope(event.get_envelope().unwrap()).unwrap();
        assert_eq!((env.name.as_str(), env.id.as_uuid()), ("Moved", id));

        let mut delete = items.delete_request();
        set_id(delete.get().init_id(), id);
        delete.send().promise.await.unwrap();
        let mut get = items.get_request();
        set_id(get.get().init_id(), id);
        let gone = get.send().promise.await.unwrap();
        assert!(!gone.get().unwrap().get_found());
        let mut by = items.get_by_uid_request();
        by.get().set_uid("u1");
        let gone = by.send().promise.await.unwrap();
        assert!(!gone.get().unwrap().get_found());
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn list_pages_through_everything_once_in_the_order_of_the_starts() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        for i in 0..230 {
            let day = 1 + i / 24;
            let hour = i % 24;
            let start = format!("2027-01-{day:02}T{hour:02}:00:00");
            create(&items, &format!("e{i:03}"), &format!("E{i:03}"), &start).await;
        }
        let (mut titles, mut cursor, mut pages) = (vec![], vec![], 0);
        loop {
            let mut call = items.list_request();
            call.get().init_cursor().set_opaque(&cursor);
            let answer = call.send().promise.await.unwrap();
            let page = answer.get().unwrap().get_page().unwrap();
            assert!(page.get_items().unwrap().len() <= 100);
            for event in page.get_items().unwrap() {
                titles.push(event_json(event)["title"].as_str().unwrap().to_string());
            }
            pages += 1;
            cursor = page.get_next().unwrap().get_opaque().unwrap().to_vec();
            if cursor.is_empty() {
                break;
            }
        }
        assert_eq!(pages, 3);
        assert_eq!(titles.len(), 230);
        assert_eq!(
            titles.iter().collect::<BTreeSet<_>>().len(),
            230,
            "each once"
        );
        let expected: Vec<String> = (0..230).map(|i| format!("E{i:03}")).collect();
        assert_eq!(titles, expected, "ordered by start");
    })
    .await;
}
