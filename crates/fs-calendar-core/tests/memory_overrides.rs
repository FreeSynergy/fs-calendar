//! Many overrides of a large event: neither writing it nor asking for it holds more than the budget.

mod counting;

use std::sync::Arc;

use counting::peak_of;
use fs_calendar_core::limits::MAX_ANSWER_BYTES;
use fs_calendar_core::*;
use fs_objects::{Store, TursoStore};
use serde_json::json;

/// A master with 4 MB of description and 2000 overrides of its first 2000 days.
fn heavy() -> Event {
    let mut e = Event::new("Heavy", "2026-01-01T09:00:00", "Etc/UTC", "PT1H");
    e.description = Some("x".repeat(4 * 1024 * 1024));
    e.recurrence_rule =
        Some(serde_json::from_value(json!({"frequency": "daily", "count": 3000})).unwrap());
    let start = chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
    for day in 1..=2000 {
        let key = (start + chrono::Days::new(day))
            .format("%Y-%m-%dT09:00:00")
            .to_string();
        e.recurrence_overrides.insert(key, json!({"title": "t"}));
    }
    e
}

#[tokio::test]
async fn writing_and_asking_hold_no_more_than_the_budget() {
    let store: Arc<dyn Store> = Arc::new(TursoStore::open_in_memory().await.unwrap());
    let r = CalendarRepo::new(store, "https://local.invalid/users/owner");
    let (made, write_peak) = peak_of(r.create(heavy())).await;
    made.unwrap();
    eprintln!("peak: writing {} MiB", write_peak >> 20);
    assert!(
        write_peak < 4 * MAX_ANSWER_BYTES,
        "writing held {write_peak} bytes"
    );

    let (window, read_peak) = peak_of(r.window(
        utc("2026-01-01T00:00:00Z").unwrap(),
        utc("2036-01-01T00:00:00Z").unwrap(),
    ))
    .await;
    let window = window.unwrap();
    eprintln!("peak: asking {} MiB", read_peak >> 20);
    assert!(
        window.instances.len() <= 9,
        "{} instances",
        window.instances.len()
    );
    assert!(
        window.damaged.iter().any(|d| d.reason.contains("cut")),
        "{:?}",
        window.damaged
    );
    assert!(
        read_peak < 4 * MAX_ANSWER_BYTES,
        "asking held {read_peak} bytes"
    );
}
