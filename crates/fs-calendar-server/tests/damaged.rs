//! What a window could not answer for, as `damaged`: a whole event, or one instance of a series.

mod common;

use common::{ME, ask_window, items_of, with_store};
use serde_json::json;

/// The contents of the instances in `[from, to)`.
async fn ask(
    items: &fs_interfaces::events_capnp::items::Client,
    from: &str,
    to: &str,
) -> capnp::Result<(Vec<()>, Vec<serde_json::Value>)> {
    let mut req = items.occurrences_request();
    req.get().init_from().set_unix_millis(common::ms(from));
    req.get().init_to().set_unix_millis(common::ms(to));
    let answer = req.send().promise.await?;
    let mut contents = vec![];
    for i in answer.get()?.get_items()? {
        contents.push(serde_json::from_str(i.get_jscalendar()?.to_str()?).unwrap());
    }
    Ok((vec![(); contents.len()], contents))
}

async fn make(items: &fs_interfaces::events_capnp::items::Client, text: &str) -> uuid::Uuid {
    common::create_value(items, &serde_json::from_str(text).unwrap()).await
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
