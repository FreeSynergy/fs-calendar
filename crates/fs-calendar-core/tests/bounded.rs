//! Reading JSON stops at the first value past the limits, before the rest is built.

mod counting;

use counting::peak_of;
use fs_calendar_core::limits::{MAX_EVENT_DEPTH, MAX_EVENT_MEMORY_BYTES, MAX_EVENT_NODES};
use fs_calendar_core::model::{Bounds, parse_bounded};
use fs_calendar_core::*;
use serde_json::json;

fn chains(count: usize, depth: usize) -> String {
    let one = format!("{}0{}", "{\"a\":".repeat(depth), "}".repeat(depth));
    format!("[{}]", vec![one; count].join(","))
}

#[tokio::test]
async fn a_document_of_millions_of_nodes_stops_early_with_a_small_peak() {
    let text = chains(340_000, 3); // about 6.8 MB
    assert!(text.len() > 6_000_000);
    let (got, peak) = peak_of(async { parse_bounded(&text, Bounds::EVENT) }).await;
    assert!(
        matches!(got, Err(Error::TooHeavy(_))),
        "stored: {}",
        got.is_ok()
    );
    // The read stops at the memory limit: its peak is that limit, not the size of the text.
    let bound = MAX_EVENT_MEMORY_BYTES + MAX_EVENT_MEMORY_BYTES / 4;
    assert!(
        peak < bound,
        "the read held {peak} bytes of a {} byte text, bound {bound}",
        text.len()
    );
}

#[test]
fn nodes_and_depth_are_counted_and_an_ordinary_event_passes() {
    let deep = format!(
        "{}0{}",
        "[".repeat(MAX_EVENT_DEPTH + 1),
        "]".repeat(MAX_EVENT_DEPTH + 1)
    );
    assert!(matches!(
        parse_bounded(&deep, Bounds::EVENT),
        Err(Error::TooHeavy(_))
    ));
    let wide = format!("[{}]", vec!["0"; MAX_EVENT_NODES + 1].join(","));
    assert!(matches!(
        parse_bounded(&wide, Bounds::EVENT),
        Err(Error::TooHeavy(_))
    ));
    let fine = r#"{"@type":"Event","uid":"u","title":"t","start":"2026-12-01T10:00:00","a":[1,2,{"b":null}]}"#;
    assert_eq!(
        parse_bounded(fine, Bounds::EVENT).unwrap(),
        serde_json::from_str::<serde_json::Value>(fine).unwrap()
    );
    assert!(matches!(
        parse_bounded("{ not json", Bounds::EVENT),
        Err(Error::Json(_))
    ));
}

#[tokio::test]
async fn an_event_past_the_limits_is_refused_when_written_in_process_too() {
    let store: std::sync::Arc<dyn fs_objects::Store> =
        std::sync::Arc::new(fs_objects::TursoStore::open_in_memory().await.unwrap());
    let r = CalendarRepo::new(store, "https://local.invalid/users/owner");
    let mut e = Event::new("Wide", "2026-12-01T10:00:00", "Etc/UTC", "PT1H");
    e.set_vendor("example.org:n", json!(vec![0; MAX_EVENT_NODES + 1]));
    assert!(matches!(r.create(e).await, Err(Error::TooHeavy(_))));
    let mut d = Event::new("Deep", "2026-12-01T10:00:00", "Etc/UTC", "PT1H");
    let deep = (0..=MAX_EVENT_DEPTH).fold(json!(0), |inner, _| json!([inner]));
    d.set_vendor("example.org:d", deep);
    assert!(matches!(r.create(d).await, Err(Error::TooHeavy(_))));
}
