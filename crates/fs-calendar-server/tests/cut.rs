//! A cut answer is sent shorter, with `cut` and `damaged` saying what is missing.

mod common;

use common::{ask_window, create_value, items_of, json, ms, with_server};
use fs_interfaces::rpc::{Problem, TOO_LARGE};

fn is(error: &capnp::Error, problem_type: &str, status: u16) {
    let p = Problem::from_error(error).expect("a typed problem");
    assert_eq!((p.problem_type.as_str(), p.status), (problem_type, status));
}

/// Twelve events every five minutes for a month are more instances than an answer holds: the answer
/// is shorter and says so (`cut`), and `damaged` names every event it is missing instances of. Every
/// other event is there whole.
#[tokio::test(flavor = "current_thread")]
async fn a_cut_answer_is_sent_shorter_and_says_so() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        let mut made = vec![];
        for i in 0..12 {
            let mut event: serde_json::Value =
                serde_json::from_str(&json(&format!("m{i}"), "Minutely", "2026-12-01T00:00:00"))
                    .unwrap();
            event["recurrenceRule"] =
                serde_json::json!({"@type": "RecurrenceRule", "frequency": "minutely", "interval": 5});
            made.push(create_value(&items, &event).await);
        }
        let asked = ask_window(&items, "2026-12-01T00:00:00Z", "2027-01-01T00:00:00Z")
            .await
            .expect("a cut answer is an answer");
        assert!(asked.cut);
        assert!(!asked.damaged.is_empty());
        for (id, reason, key) in &asked.damaged {
            assert!(made.contains(id), "{id}");
            assert!(reason.starts_with("cut: "), "{reason}");
            assert_eq!(key, "", "a cut event has no key");
        }
        let missing: Vec<_> = asked.damaged.iter().map(|d| d.0).collect();
        let whole = 31 * 288; // 8 928 starts a day of five minutes, over 31 days
        for id in &made {
            let n = asked.objects.iter().filter(|o| *o == id).count();
            if missing.contains(id) {
                assert!(n < whole, "{id}: {n} of a cut event");
            } else {
                assert_eq!(n, whole, "{id} is there whole");
            }
        }
        assert!(asked.objects.len() < 12 * whole);
        let small = ask_window(&items, "2026-12-01T00:00:00Z", "2026-12-01T01:00:00Z")
            .await
            .expect("an hour fits");
        // every instance lasts an hour: 23 starts of each event overlap the hour asked for
        assert_eq!(small.objects.len(), 12 * 23);
        assert!(!small.cut && small.damaged.is_empty());
    })
    .await;
}

/// Two hundred days of an event every five minutes are more than one event may give: that event is
/// named in `damaged` (with its id), what is returned of it stops at its limit, and the rest answers.
#[tokio::test(flavor = "current_thread")]
async fn one_event_that_is_too_much_alone_does_not_refuse_the_others() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        let mut event: serde_json::Value =
            serde_json::from_str(&json("one", "Often", "2026-12-01T00:00:00")).unwrap();
        event["recurrenceRule"] =
            serde_json::json!({"@type": "RecurrenceRule", "frequency": "minutely", "interval": 5});
        let often = create_value(&items, &event).await;
        common::create(&items, "lunch", "Lunch", "2027-03-01T12:00:00").await;
        let mut req = items.occurrences_request();
        req.get()
            .init_from()
            .set_unix_millis(ms("2026-12-01T00:00:00Z"));
        req.get()
            .init_to()
            .set_unix_millis(ms("2027-06-20T00:00:00Z"));
        let answer = req
            .send()
            .promise
            .await
            .expect("the rest of the calendar answers");
        let found: Vec<String> = answer
            .get()
            .unwrap()
            .get_items()
            .unwrap()
            .iter()
            .map(|i| i.get_jscalendar().unwrap().to_string().unwrap())
            .collect();
        assert!(found.iter().any(|j| j.contains("Lunch")), "Lunch is there");
        assert!(found.len() > 5_000);
        let asked = ask_window(&items, "2026-12-01T00:00:00Z", "2027-06-20T00:00:00Z")
            .await
            .unwrap();
        // It is named, by its id, and the answer is cut.
        assert_eq!(asked.damaged.len(), 1, "{:?}", asked.damaged);
        assert_eq!(asked.damaged[0].0, often);
        assert!(
            asked.damaged[0].1.starts_with("cut: "),
            "{:?}",
            asked.damaged
        );
        assert!(
            asked.cut,
            "the contract: cut when any event is cut at a bound"
        );
        let n = asked.objects.iter().filter(|o| **o == often).count();
        assert!((5_000..=10_000).contains(&n), "it stops at its limit: {n}");
    })
    .await;
}

/// Six million small numbers are 12 MB as text and need far more in memory: refused as too large.
#[tokio::test(flavor = "current_thread")]
async fn an_event_of_millions_of_small_values_is_too_large_to_hold() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        let mut event: serde_json::Value =
            serde_json::from_str(&json("heavy", "Heavy", "2026-12-01T10:00:00")).unwrap();
        event["example.org:numbers"] = serde_json::json!(vec![0; 6_000_000]);
        let mut call = items.create_request();
        call.get().set_jscalendar(event.to_string().as_str());
        is(&call.send().promise.await.err().unwrap(), TOO_LARGE, 413);
    })
    .await;
}

/// A text of millions of small values stops at the first value past the limits, before it is built.
#[tokio::test(flavor = "current_thread")]
async fn a_request_of_millions_of_nodes_is_refused_and_the_connection_lives() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        let chain = "{\"a\":{\"a\":{\"a\":0}}}";
        let numbers = vec![chain; 340_000].join(",");
        let text = format!(
            r#"{{"@type":"Event","version":"2.0","uid":"chains","title":"C","start":"2026-12-01T10:00:00","example.org:c":[{numbers}]}}"#
        );
        assert!(text.len() > 6_000_000);
        let mut call = items.create_request();
        call.get().set_jscalendar(text.as_str());
        is(&call.send().promise.await.err().unwrap(), TOO_LARGE, 413);
        items.list_request().send().promise.await.expect("still connected");
    })
    .await;
}
