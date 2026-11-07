//! A value that escapes to more than a message is answered with a short problem, never the value.

mod common;

use common::{create, items_of, with_server};
use fs_interfaces::rpc::Problem;

const MIB: usize = 1024 * 1024;
const INVALID: &str = "https://freesynergy.net/problems/calendar/invalid";

/// Control characters escape to six bytes each: 90 MiB in one error, without the short quote and the cap.
#[tokio::test(flavor = "current_thread")]
async fn a_value_that_escapes_to_more_than_a_message_is_answered() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        create(&items, "uid-1", "First", "2026-12-01T10:00:00").await;
        let bad = "\u{7f}".repeat(15 * MIB);
        for member in [
            "start",
            "duration",
            "timeZone",
            "endTimeZone",
            "updated",
            "created",
            "status",
            "recurrenceId",
            "freeBusyStatus",
            "privacy",
        ] {
            let event = serde_json::json!({
                "@type": "Event", "version": "2.0", "uid": "u", "title": "t",
                "start": "2026-12-01T10:00:00", member: bad,
            });
            let mut call = items.create_request();
            call.get().set_jscalendar(event.to_string().as_str());
            let error = call
                .send()
                .promise
                .await
                .err()
                .unwrap_or_else(|| panic!("{member} was accepted"));
            let problem = Problem::from_error(&error).unwrap();
            assert_eq!(
                (problem.problem_type.as_str(), problem.status),
                (INVALID, 422),
                "{member}"
            );
            assert!(
                problem.detail.len() < 4096,
                "{member}: {} bytes",
                problem.detail.len()
            );
            assert!(
                problem.detail.contains(member),
                "{member}: {}",
                problem.detail
            );
        }
        let list = items
            .list_request()
            .send()
            .promise
            .await
            .expect("still connected");
        assert_eq!(
            list.get()
                .unwrap()
                .get_page()
                .unwrap()
                .get_items()
                .unwrap()
                .len(),
            1
        );
    })
    .await;
}

/// The same inside a rule and an override.
#[tokio::test(flavor = "current_thread")]
async fn a_rule_or_an_override_with_such_a_value_is_answered_short_too() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        let bad = "\u{7f}".repeat(15 * MIB);
        let base = serde_json::json!({
            "@type": "Event", "version": "2.0", "uid": "u", "title": "t",
            "start": "2026-12-01T10:00:00", "timeZone": "Etc/UTC", "duration": "PT1H",
        });
        let mut rule = base.clone();
        rule["recurrenceRule"] = serde_json::json!({"@type": "RecurrenceRule", "frequency": bad});
        let mut until = base.clone();
        until["recurrenceRule"] =
            serde_json::json!({"@type": "RecurrenceRule", "frequency": "daily", "until": bad});
        let mut key = base.clone();
        key["recurrenceOverrides"] = serde_json::json!({ bad.clone(): {"title": "x"} });
        let mut patch = base.clone();
        patch["recurrenceOverrides"] = serde_json::json!({"2026-12-02T10:00:00": {"start": bad}});
        for (what, event) in [
            ("frequency", rule),
            ("until", until),
            ("key", key),
            ("patch", patch),
        ] {
            let mut call = items.create_request();
            call.get().set_jscalendar(event.to_string().as_str());
            let error = call.send().promise.await.err().unwrap();
            let problem = Problem::from_error(&error).unwrap();
            assert_eq!(
                problem.status,
                422,
                "{what}: {}",
                &problem.detail[..problem.detail.len().min(200)]
            );
            assert!(
                problem.detail.len() < 4096,
                "{what}: {} bytes",
                problem.detail.len()
            );
        }
        items
            .list_request()
            .send()
            .promise
            .await
            .expect("still connected");
    })
    .await;
}
