//! `occurrences`: a series expanded at query time, with its overrides, over a real socket.

mod common;

use common::{ME, ask_window, items_of, ms, with_server, with_store};
use fs_interfaces::events_capnp::items;
use fs_interfaces::rpc::{Problem, read_uuid};
use serde_json::{Value, json};

const SERIES: &str = r#"{"@type":"Event","version":"2.0","uid":"w1","title":"Weekly","start":"2026-11-03T10:00:00","timeZone":"Europe/Berlin","duration":"PT1H","recurrenceRule":{"@type":"RecurrenceRule","frequency":"weekly","count":4},"recurrenceOverrides":{"2026-11-10T10:00:00":{"excluded":true},"2026-11-17T10:00:00":{"title":"Moved","start":"2026-11-17T14:00:00"},"2026-11-02T08:00:00":{"title":"Before the start"}}}"#;

async fn make(items: &items::Client, text: &str) -> uuid::Uuid {
    let mut req = items.create_request();
    req.get().set_jscalendar(text);
    let answer = req.send().promise.await.unwrap();
    let env = answer
        .get()
        .unwrap()
        .get_event()
        .unwrap()
        .get_envelope()
        .unwrap();
    fs_interfaces::rpc::read_envelope(env).unwrap().id.as_uuid()
}

type Row = (i64, i64, bool, String);

async fn ask(items: &items::Client, from: &str, to: &str) -> capnp::Result<(Vec<Row>, Vec<Value>)> {
    let mut req = items.occurrences_request();
    req.get().init_from().set_unix_millis(ms(from));
    req.get().init_to().set_unix_millis(ms(to));
    let answer = req.send().promise.await?;
    let (mut rows, mut contents) = (vec![], vec![]);
    for i in answer.get()?.get_items()? {
        let content: Value = serde_json::from_str(i.get_jscalendar()?.to_str()?).unwrap();
        let object = read_uuid(i.get_object()?).unwrap().to_string();
        rows.push((
            i.get_start()?.get_unix_millis(),
            i.get_end()?.get_unix_millis(),
            i.get_overridden(),
            object,
        ));
        contents.push(content);
    }
    Ok((rows, contents))
}

#[tokio::test(flavor = "current_thread")]
async fn a_series_comes_expanded_with_its_overrides_applied() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        let id = make(&items, SERIES).await.to_string();
        let (rows, contents) = ask(&items, "2026-11-01T00:00:00Z", "2026-12-01T00:00:00Z")
            .await
            .unwrap();
        let expect = [
            (
                "2026-11-02T07:00:00Z",
                "2026-11-02T07:00:00Z",
                true,
                "Before the start",
            ),
            (
                "2026-11-03T09:00:00Z",
                "2026-11-03T10:00:00Z",
                false,
                "Weekly",
            ),
            (
                "2026-11-17T13:00:00Z",
                "2026-11-17T14:00:00Z",
                true,
                "Moved",
            ),
            (
                "2026-11-24T09:00:00Z",
                "2026-11-24T10:00:00Z",
                false,
                "Weekly",
            ),
        ];
        assert_eq!(rows.len(), expect.len(), "{rows:?}");
        for (i, (s, e, over, title)) in expect.iter().enumerate() {
            assert_eq!(
                (rows[i].0, rows[i].2, rows[i].3.as_str()),
                (ms(s), *over, id.as_str()),
                "{i}"
            );
            if *title != "Before the start" {
                assert_eq!(rows[i].1, ms(e), "{i}");
            }
            assert_eq!(contents[i]["title"], *title, "{i}");
        }
        assert_eq!(contents[2]["recurrenceId"], "2026-11-17T10:00:00");
        assert!(
            contents[2].get("recurrenceRule").is_none(),
            "the instance, not the series"
        );
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn the_window_is_half_open_and_an_empty_one_is_empty() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        make(&items, SERIES).await;
        // 09:00Z to 10:00Z: touching at the end is not overlapping
        let (rows, _) = ask(&items, "2026-11-03T10:00:00Z", "2026-11-03T11:00:00Z")
            .await
            .unwrap();
        assert!(rows.is_empty(), "{rows:?}");
        let (rows, _) = ask(&items, "2026-11-03T09:59:59Z", "2026-11-03T11:00:00Z")
            .await
            .unwrap();
        assert_eq!(rows.len(), 1);
        let (rows, _) = ask(&items, "2026-11-03T09:00:00Z", "2026-11-03T09:00:00Z")
            .await
            .unwrap();
        assert!(rows.is_empty());
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn a_window_that_ends_before_it_starts_is_an_invalid_request() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        let error = ask(&items, "2026-11-05T00:00:00Z", "2026-11-04T00:00:00Z")
            .await
            .unwrap_err();
        let problem = Problem::from_error(&error).unwrap();
        assert_eq!(
            (problem.problem_type.as_str(), problem.status),
            (fs_interfaces::rpc::INVALID_REQUEST, 422)
        );
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn a_broken_override_in_the_store_costs_its_instance_not_the_answer() {
    with_store(|root, store| async move {
        let items = items_of(&root).await;
        let daily = |uid: &str, title: &str| {
            json!({"@type":"Event","version":"2.0","uid":uid,"title":title,"start":"2026-11-03T10:00:00",
                "timeZone":"Etc/UTC","duration":"PT1H","recurrenceRule":{"@type":"RecurrenceRule","frequency":"daily"}})
        };
        make(&items, &daily("fine", "Fine").to_string()).await;
        let broken = make(&items, &daily("broken", "Broken").to_string()).await;
        let mut object = store.get(broken.to_string().parse().unwrap()).await.unwrap().unwrap();
        object.content["recurrenceOverrides"] = json!({"2026-11-05T10:00:00": {"duration": "nonsense"}});
        store.put(object, ME).await.unwrap();
        let (rows, contents) = ask(&items, "2026-11-04T00:00:00Z", "2026-11-07T00:00:00Z").await.unwrap();
        let count = |title: &str| contents.iter().filter(|c| c["title"] == title).count();
        assert_eq!((count("Fine"), count("Broken")), (3, 2), "{rows:?}");
        // The damage is told, by event and by the key of the instance; nothing is cut.
        let asked = ask_window(&items, "2026-11-04T00:00:00Z", "2026-11-07T00:00:00Z")
            .await
            .unwrap();
        assert!(!asked.cut);
        assert_eq!(asked.damaged.len(), 1, "{:?}", asked.damaged);
        let (id, reason, key) = &asked.damaged[0];
        assert_eq!((id, key.as_str()), (&broken, "2026-11-05T10:00:00"));
        assert!(!reason.is_empty() && !reason.starts_with("cut: "), "{reason}");

        // A whole event that does not read is missing from the items and has an empty key.
        let mut object = store.get(broken.to_string().parse().unwrap()).await.unwrap().unwrap();
        object.content["recurrenceRule"] = json!({"@type": "RecurrenceRule", "frequency": "nonsense"});
        object.content["recurrenceOverrides"] = json!({});
        store.put(object, ME).await.unwrap();
        let asked = ask_window(&items, "2026-11-04T00:00:00Z", "2026-11-07T00:00:00Z")
            .await
            .unwrap();
        assert_eq!(asked.damaged.len(), 1, "{:?}", asked.damaged);
        assert_eq!((asked.damaged[0].0, asked.damaged[0].2.as_str()), (broken, ""));
        assert!(!asked.objects.contains(&broken) && !asked.cut);
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn a_window_from_the_smallest_to_the_largest_instant_answers() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        make(&items, SERIES).await;
        let mut req = items.occurrences_request();
        req.get()
            .init_from()
            .set_unix_millis(chrono::DateTime::<chrono::Utc>::MIN_UTC.timestamp_millis());
        req.get()
            .init_to()
            .set_unix_millis(chrono::DateTime::<chrono::Utc>::MAX_UTC.timestamp_millis());
        let answer = req
            .send()
            .promise
            .await
            .expect("the extreme window answers");
        assert_eq!(answer.get().unwrap().get_items().unwrap().len(), 4);
        let mut beyond = items.occurrences_request();
        beyond.get().init_from().set_unix_millis(i64::MIN);
        beyond.get().init_to().set_unix_millis(i64::MAX);
        let error = beyond.send().promise.await.err().unwrap();
        assert_eq!(Problem::from_error(&error).unwrap().status, 422);
    })
    .await;
}
