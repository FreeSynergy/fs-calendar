//! A cut answer is refused, never sent shorter.

mod common;

use common::{items_of, json, ms, with_server};
use fs_interfaces::rpc::{Problem, TOO_LARGE};

fn is(error: &capnp::Error, problem_type: &str, status: u16) {
    let p = Problem::from_error(error).expect("a typed problem");
    assert_eq!((p.problem_type.as_str(), p.status), (problem_type, status));
}

/// Twelve minutely events for a week are more instances than an event may give: the consumer is told
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
                serde_json::json!({"@type": "RecurrenceRule", "frequency": "minutely"});
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
            .set_unix_millis(ms("2026-12-08T00:00:00Z"));
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
        // every instance lasts an hour, so 119 of each event overlap the hour asked for
        assert_eq!(answer.get().unwrap().get_items().unwrap().len(), 12 * 119);
    })
    .await;
}
