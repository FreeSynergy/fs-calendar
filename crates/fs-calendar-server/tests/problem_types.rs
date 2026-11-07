//! Problems with a type of their own: what a consumer must tell apart from a plain invalid request.

mod common;

use std::sync::Arc;

use common::{ME, items_of, with_provider};
use fs_calendar_core::CalendarRepo;
use fs_interfaces::rpc::Problem;
use fs_store::{Store, TursoStore};

fn is(error: &capnp::Error, problem_type: &str, status: u16) {
    let p = Problem::from_error(error).expect("a typed problem");
    assert_eq!((p.problem_type.as_str(), p.status), (problem_type, status));
}

#[tokio::test(flavor = "current_thread")]
async fn a_marked_event_in_a_store_that_is_not_encrypted_and_a_dense_rule_have_their_own_types() {
    let store: Arc<dyn Store> = Arc::new(
        TursoStore::open_with(":memory:", fs_store::OpenOptions::default())
            .await
            .unwrap(),
    );
    let repo = Arc::new(CalendarRepo::new(store, ME));
    with_provider(repo.clone(), repo, |root| async move {
        let items = items_of(&root).await;
        let mut marked = items.create_request();
        marked.get().set_jscalendar(
            r#"{"@type":"Event","version":"2.0","uid":"m","title":"M","start":"2026-12-01T10:00:00","freesynergy.net:sensitive":["dpv:Health"]}"#,
        );
        is(
            &marked.send().promise.await.err().unwrap(),
            "https://freesynergy.net/problems/events/encryption-required",
            409,
        );
        let dense = serde_json::json!({"@type":"Event","version":"2.0","uid":"d","title":"D",
            "start":"2026-12-01T10:00:00","recurrenceRule":{"@type":"RecurrenceRule","frequency":"yearly",
            "byMonth":(1..=12).map(|m| m.to_string()).collect::<Vec<_>>(),"byMonthDay":(1..=31).collect::<Vec<_>>(),
            "byHour":(0..24).collect::<Vec<_>>(),"byMinute":(0..60).collect::<Vec<_>>(),"bySecond":(0..60).collect::<Vec<_>>()}});
        let mut rule = items.create_request();
        rule.get().set_jscalendar(dense.to_string().as_str());
        is(
            &rule.send().promise.await.err().unwrap(),
            "https://freesynergy.net/problems/calendar/rule-too-expensive",
            422,
        );
    })
    .await;
}
