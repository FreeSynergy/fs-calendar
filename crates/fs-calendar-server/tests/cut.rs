//! A cut answer is refused, never sent shorter.

mod common;

use common::{items_of, json, ms, with_server};
use fs_interfaces::rpc::{Problem, TOO_LARGE};

fn is(error: &capnp::Error, problem_type: &str, status: u16) {
    let p = Problem::from_error(error).expect("a typed problem");
    assert_eq!((p.problem_type.as_str(), p.status), (problem_type, status));
}

/// Twelve events every five minutes for a month are more instances than an event may give: the consumer is told
/// to ask for less, it does not get a silently shorter answer.
#[tokio::test(flavor = "current_thread")]
async fn a_cut_answer_is_refused_not_sent_shorter() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        for i in 0..12 {
            let mut event: serde_json::Value =
                serde_json::from_str(&json(&format!("m{i}"), "Minutely", "2026-12-01T00:00:00"))
                    .unwrap();
            event["recurrenceRule"] =
                serde_json::json!({"@type": "RecurrenceRule", "frequency": "minutely", "interval": 5});
            let mut call = items.create_request();
            call.get().set_jscalendar(event.to_string().as_str());
            call.send().promise.await.unwrap();
        }
        let mut req = items.occurrences_request();
        req.get()
            .init_from()
            .set_unix_millis(ms("2026-12-01T00:00:00Z"));
        req.get()
            .init_to()
            .set_unix_millis(ms("2027-01-01T00:00:00Z"));
        is(&req.send().promise.await.err().unwrap(), TOO_LARGE, 413);
        let mut small = items.occurrences_request();
        small
            .get()
            .init_from()
            .set_unix_millis(ms("2026-12-01T00:00:00Z"));
        small
            .get()
            .init_to()
            .set_unix_millis(ms("2026-12-01T01:00:00Z"));
        let answer = small.send().promise.await.expect("an hour fits");
        // every instance lasts an hour: 23 starts of each event overlap the hour asked for
        assert_eq!(answer.get().unwrap().get_items().unwrap().len(), 12 * 23);
    })
    .await;
}

/// Two hundred days of an event every five minutes are more than one event may give: that event is
/// named (on stderr, with its id), what is returned of it stops at its limit, and the rest answers.
#[tokio::test(flavor = "current_thread")]
async fn one_event_that_is_too_much_alone_does_not_refuse_the_others() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        let mut event: serde_json::Value =
            serde_json::from_str(&json("one", "Often", "2026-12-01T00:00:00")).unwrap();
        event["recurrenceRule"] =
            serde_json::json!({"@type": "RecurrenceRule", "frequency": "minutely", "interval": 5});
        let mut call = items.create_request();
        call.get().set_jscalendar(event.to_string().as_str());
        call.send().promise.await.unwrap();
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
