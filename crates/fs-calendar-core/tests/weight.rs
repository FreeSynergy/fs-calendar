//! The estimate of memory is never below the memory: a single entry of an object takes hundreds of bytes.

mod counting;

use counting::peak_of;
use fs_calendar_core::memory_weight;
use serde_json::{Value, json};

/// The memory a copy of the value takes, measured by cloning it.
async fn real(value: &Value) -> usize {
    let (copy, peak) = peak_of(async { value.clone() }).await;
    drop(copy);
    peak
}

fn shapes() -> Vec<(&'static str, Value)> {
    let chain = |depth: usize| (0..depth).fold(json!(0), |inner, _| json!({ "a": inner }));
    vec![
        (
            "objects of one entry",
            Value::Array((0..30_000).map(|_| json!({"a": 0})).collect()),
        ),
        (
            "chains of depth 10",
            Value::Array((0..3_000).map(|_| chain(10)).collect()),
        ),
        ("small numbers", json!(vec![0; 300_000])),
        ("text", json!("x".repeat(5_000_000))),
        (
            "keys",
            Value::Object(
                (0..20_000)
                    .map(|i| (format!("key-{i}"), json!(i)))
                    .collect(),
            ),
        ),
    ]
}

#[tokio::test]
async fn the_estimate_is_not_below_what_a_copy_takes() {
    for (what, value) in shapes() {
        let (weight, real) = (memory_weight(&value), real(&value).await);
        assert!(
            weight >= real,
            "{what}: estimated {weight}, a copy takes {real}"
        );
        assert!(
            weight < 3 * real,
            "{what}: estimated {weight} is far above {real}"
        );
    }
}
