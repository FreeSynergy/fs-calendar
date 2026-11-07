//! What a question builds is bounded in bytes before it is built: peak memory, not only the answer.

mod counting;

use std::sync::Arc;

use counting::peak_of;
use fs_calendar_core::limits::MAX_ANSWER_BYTES;
use fs_calendar_core::*;
use fs_objects::{Store, TursoStore};

#[tokio::test]
async fn a_long_series_of_a_large_event_never_holds_more_than_the_budget() {
    let store: Arc<dyn Store> = Arc::new(TursoStore::open_in_memory().await.unwrap());
    let r = CalendarRepo::new(store, "https://local.invalid/users/owner");
    let mut e = Event::new("Daily", "2026-01-01T09:00:00", "Etc/UTC", "PT1H");
    e.description = Some("x".repeat(20 * 1024));
    e.recurrence_rule =
        Some(serde_json::from_value(serde_json::json!({"frequency": "daily"})).unwrap());
    r.create(e).await.unwrap();

    let (window, peak) = peak_of(r.window(
        utc("2026-01-01T00:00:00Z").unwrap(),
        utc("2176-01-01T00:00:00Z").unwrap(),
    ))
    .await;
    let window = window.unwrap();
    let held: usize = window
        .instances
        .iter()
        .map(|i| i.occurrence.event.to_value().to_string().len())
        .sum();
    assert!(held <= MAX_ANSWER_BYTES, "{held} bytes of instances");
    assert!(
        window.damaged.iter().any(|d| d.reason.contains("cut")),
        "{:?}",
        window.damaged
    );
    assert!(
        peak < 4 * MAX_ANSWER_BYTES,
        "peak {peak} bytes for an answer of {held}"
    );
}
