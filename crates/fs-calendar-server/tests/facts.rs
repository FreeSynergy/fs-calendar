//! The facts behind a problem, over the wire: what a client used to cut out of the detail text.

mod common;

use common::{create, items_of, json, set_id, with_server};
use fs_interfaces::rpc::Problem;

const CONFLICT: &str = "https://freesynergy.net/problems/calendar/conflict";

#[tokio::test(flavor = "current_thread")]
async fn a_stale_revision_names_the_event_and_the_kind_without_the_text() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        let id = create(&items, "f1", "First", "2026-12-01T10:00:00").await;
        let mut stale = items.update_if_request();
        set_id(stale.get().init_id(), id);
        stale.get().set_revision(1);
        stale
            .get()
            .set_jscalendar(json("f1", "Late", "2026-12-01T10:00:00").as_str());
        let error = stale.send().promise.await.err().unwrap();
        let problem = Problem::from_error(&error).unwrap();
        assert_eq!(
            (problem.problem_type.as_str(), problem.status),
            (CONFLICT, 409)
        );
        assert_eq!(problem.ext("id"), Some(id.to_string().as_str()));
        assert_eq!(problem.ext("kind"), Some("event"));
        assert!(
            problem.detail.starts_with("conflict:"),
            "{}",
            problem.detail
        );
    })
    .await;
}

async fn refused(items: &fs_interfaces::events_capnp::items::Client, text: &str) -> Problem {
    let mut call = items.create_request();
    call.get().set_jscalendar(text);
    Problem::from_error(&call.send().promise.await.err().expect("refused")).unwrap()
}

const INVALID: &str = "https://freesynergy.net/problems/calendar/invalid";
const EXPENSIVE: &str = "https://freesynergy.net/problems/calendar/rule-too-expensive";

#[tokio::test(flavor = "current_thread")]
async fn an_unknown_event_is_named_by_id_and_kind() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        let unknown = uuid::Uuid::now_v7();
        let mut update = items.update_request();
        set_id(update.get().init_id(), unknown);
        update
            .get()
            .set_jscalendar(json("f2", "x", "2026-12-01T10:00:00").as_str());
        let problem = Problem::from_error(&update.send().promise.await.err().unwrap()).unwrap();
        assert_eq!(problem.status, 404);
        assert_eq!(problem.ext("id"), Some(unknown.to_string().as_str()));
        assert_eq!(problem.ext("kind"), Some("event"));
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn what_is_wrong_with_an_event_is_an_issue_and_a_kind_not_only_text() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        let start = refused(&items, &json("f3", "T", "not a time")).await;
        assert_eq!(start.problem_type, INVALID);
        let issues: Vec<_> = start.ext_all("issue").collect();
        assert_eq!(issues.len(), 1, "{start:?}");
        assert!(issues[0].starts_with("/start:"), "{issues:?}");

        let text = r#"{"@type":"Event","version":"2.0","uid":"f4","title":"T","start":"2026-12-01T10:00:00","duration":"nonsense"}"#;
        let duration = refused(&items, text).await;
        assert_eq!(duration.problem_type, INVALID);
        assert!(duration.ext("issue").is_some(), "{duration:?}");

        let mut call = items.import_text_request();
        call.get().set_media_type("text/plain");
        call.get().set_text("x");
        let media = Problem::from_error(&call.send().promise.await.err().unwrap()).unwrap();
        assert_eq!(media.ext("kind"), Some("media-type"), "{media:?}");
        assert_eq!(media.ext("issue"), Some("text/plain"));
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn a_rule_that_costs_too_much_says_why_as_an_issue() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        let dense = serde_json::json!({"@type":"Event","version":"2.0","uid":"d","title":"D",
            "start":"2026-12-01T10:00:00","recurrenceRule":{"@type":"RecurrenceRule","frequency":"yearly",
            "byMonth":(1..=12).map(|m| m.to_string()).collect::<Vec<_>>(),"byMonthDay":(1..=31).collect::<Vec<_>>(),
            "byHour":(0..24).collect::<Vec<_>>(),"byMinute":(0..60).collect::<Vec<_>>(),"bySecond":(0..60).collect::<Vec<_>>()}});
        let problem = refused(&items, &dense.to_string()).await;
        assert_eq!(problem.problem_type, EXPENSIVE);
        assert!(problem.ext("issue").is_some_and(|i| !i.is_empty()), "{problem:?}");
    })
    .await;
}
