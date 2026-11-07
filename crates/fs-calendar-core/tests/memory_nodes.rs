//! The budget counts memory, not JSON: a value of many small nodes is much larger in memory than as text.

mod counting;

use std::sync::Arc;

use counting::peak_of;
use fs_calendar_core::limits::{MAX_ANSWER_BYTES, MAX_EVENT_MEMORY_BYTES};
use fs_calendar_core::*;
use fs_store::{Store, TursoStore};
use serde_json::json;

/// The heaviest shapes the limits let in: 24 MiB of estimated memory in each.
fn shapes() -> Vec<(&'static str, serde_json::Value)> {
    let chain = |depth: usize| (0..depth).fold(json!(0), |inner, _| json!({ "a": inner }));
    vec![
        (
            "objects of one entry",
            serde_json::Value::Array((0..30_000).map(|_| json!({"a": 0})).collect()),
        ),
        (
            "chains of depth 10",
            serde_json::Value::Array((0..3_000).map(|_| chain(10)).collect()),
        ),
        ("small numbers", json!(vec![0; 90_000])),
        (
            "arrays of one element",
            serde_json::Value::Array((0..49_000).map(|_| json!([0])).collect()),
        ),
        (
            "chains of arrays, depth 28",
            serde_json::Value::Array(
                (0..3_000)
                    .map(|_| (0..28).fold(json!(0), |inner, _| json!([inner])))
                    .collect(),
            ),
        ),
    ]
}

fn event(uid: &str, extra: &serde_json::Value) -> Event {
    let mut e = Event::new(uid, "2026-01-01T09:00:00", "Etc/UTC", "PT1H");
    e.uid = uid.into();
    e.set_vendor("example.org:extra", extra.clone());
    e.recurrence_rule = Some(serde_json::from_value(json!({"frequency": "daily"})).unwrap());
    e
}

async fn repo() -> CalendarRepo {
    let store: Arc<dyn Store> = Arc::new(TursoStore::open_in_memory().await.unwrap());
    CalendarRepo::new(store, "https://local.invalid/users/owner")
}

/// The most memory a question holds, as measured here: three times the budget of the answer plus three
/// copies of the heaviest event (`DESIGN.md` has the measured values).
fn bound(weight: usize) -> usize {
    3 * MAX_ANSWER_BYTES + 3 * weight
}

#[tokio::test]
async fn the_heaviest_events_the_limits_allow_are_written_and_asked_within_the_bound() {
    for (what, extra) in shapes() {
        let r = repo().await;
        let weight = memory_weight(&event("w", &extra).to_value());
        assert!(weight <= MAX_EVENT_MEMORY_BYTES, "{what}: {weight}");
        let (made, write_peak) = peak_of(r.create(event("w", &extra))).await;
        made.unwrap_or_else(|e| panic!("{what}: {e}"));
        let (window, read_peak) = peak_of(r.window(
            utc("2026-01-01T00:00:00Z").unwrap(),
            utc("2026-02-01T00:00:00Z").unwrap(),
        ))
        .await;
        let window = window.unwrap();
        eprintln!(
            "{what}: weight {} MiB, writing {} MiB, asking {} MiB, {} instances",
            weight >> 20,
            write_peak >> 20,
            read_peak >> 20,
            window.instances.len()
        );
        assert!(
            window.damaged.iter().any(|d| d.reason.contains("cut")),
            "{what}: {:?}",
            window.damaged
        );
        assert!(
            read_peak < bound(weight),
            "{what}: asking held {read_peak}, bound {}",
            bound(weight)
        );
        assert!(
            write_peak < bound(weight),
            "{what}: writing held {write_peak}, bound {}",
            bound(weight)
        );
    }
}

#[tokio::test]
async fn forty_small_events_of_many_small_values_are_asked_within_the_bound() {
    let objects = serde_json::Value::Array((0..10_000).map(|_| json!({"a": 0})).collect());
    let chains = serde_json::Value::Array(
        (0..3_300)
            .map(|_| (0..28).fold(json!(0), |inner, _| json!([inner])))
            .collect(),
    );
    for (what, extra) in [("objects", objects), ("array chains", chains)] {
        let r = repo().await;
        let weight = memory_weight(&event("w", &extra).to_value());
        for i in 0..40 {
            r.create(event(&format!("e{i}"), &extra)).await.unwrap();
        }
        let (window, peak) = peak_of(r.window(
            utc("2026-01-01T00:00:00Z").unwrap(),
            utc("2026-02-01T00:00:00Z").unwrap(),
        ))
        .await;
        let window = window.unwrap();
        eprintln!(
            "40 events of {what}: weight {} MiB each, asking {} MiB, {} instances",
            weight >> 20,
            peak >> 20,
            window.instances.len()
        );
        // The store keeps about 6 MiB for each distinct page query of rows this heavy (measured with
        // `fs-store` v0.2.0: sixty queries of one row each hold 330 MiB, also without this crate); that
        // is its memory, not an instance, and it is added here and named in `DESIGN.md`.
        let store_keeps = 40 * 6 * 1024 * 1024;
        assert!(
            peak < bound(weight) + store_keeps,
            "{what}: asking held {peak}, bound {}",
            bound(weight) + store_keeps
        );
    }
}
