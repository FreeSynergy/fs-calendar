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
