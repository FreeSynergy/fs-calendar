//! An event that would need more memory than the limit is refused when written (own file: the
//! allocator counters of `memory_nodes.rs` must not see this test).

use std::sync::Arc;

use fs_calendar_core::*;
use fs_objects::{Store, TursoStore};
use serde_json::json;

#[tokio::test]
async fn an_event_that_would_need_more_memory_than_the_limit_is_refused_when_written() {
    let store: Arc<dyn Store> = Arc::new(TursoStore::open_in_memory().await.unwrap());
    let r = CalendarRepo::new(store, "https://local.invalid/users/owner");
    // 6 million numbers are 12 MB as text and about 190 MB in memory
    let got = r.create(heavy()).await;
    assert!(
        matches!(got, Err(Error::TooHeavy(_))),
        "stored: {}",
        got.is_ok()
    );
}

fn heavy() -> Event {
    let mut e = Event::new("Nodes", "2026-01-01T09:00:00", "Etc/UTC", "PT1H");
    e.set_vendor("example.org:numbers", json!(vec![0; 6_000_000]));
    e
}
