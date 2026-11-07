//! The budget counts memory, not JSON: a value of many small nodes is much larger in memory than as text.

mod counting;

use std::sync::Arc;

use counting::peak_of;
use fs_calendar_core::limits::MAX_ANSWER_BYTES;
use fs_calendar_core::*;
use fs_store::{Store, TursoStore};
use serde_json::json;

fn many_nodes(count: usize) -> Event {
    let mut e = Event::new("Nodes", "2026-01-01T09:00:00", "Etc/UTC", "PT1H");
    e.set_vendor("example.org:numbers", json!(vec![0; count]));
    e.recurrence_rule = Some(serde_json::from_value(json!({"frequency": "daily"})).unwrap());
    e
}

async fn repo() -> CalendarRepo {
    let store: Arc<dyn Store> = Arc::new(TursoStore::open_in_memory().await.unwrap());
    CalendarRepo::new(store, "https://local.invalid/users/owner")
}

#[tokio::test]
async fn an_event_of_300_000_small_nodes_is_asked_within_the_budget() {
    let r = repo().await;
    let (made, write_peak) = peak_of(r.create(many_nodes(300_000))).await;
    made.unwrap();
    let (window, read_peak) = peak_of(r.window(
        utc("2026-01-01T00:00:00Z").unwrap(),
        utc("2026-02-01T00:00:00Z").unwrap(),
    ))
    .await;
    let window = window.unwrap();
    eprintln!(
        "nodes: writing {} MiB, asking {} MiB, {} instances",
        write_peak >> 20,
        read_peak >> 20,
        window.instances.len()
    );
    assert!(window.damaged.iter().any(|d| d.cut), "{:?}", window.damaged);
    assert!(
        read_peak < 4 * MAX_ANSWER_BYTES,
        "asking held {read_peak} bytes"
    );
    assert!(
        write_peak < 4 * MAX_ANSWER_BYTES,
        "writing held {write_peak} bytes"
    );
}
