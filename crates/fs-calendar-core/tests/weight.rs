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
            weight + 64 * 1024 >= real, // a few KiB of the harness may be counted
            "{what}: estimated {weight}, a copy takes {real}"
        );
        assert!(
            weight < 4 * real,
            "{what}: estimated {weight} is far above {real}"
        );
    }
}

/// The memory of a value built by `parse_bounded` at its highest, including what its arrays held while
/// they grew: the estimate must be above that too, not only above a clone.
async fn peak_of_reading(text: &str) -> usize {
    let huge = fs_calendar_core::model::Bounds {
        nodes: usize::MAX,
        depth: 200,
        memory: usize::MAX,
    };
    let (value, peak) =
        peak_of(async { fs_calendar_core::model::parse_bounded(text, huge).unwrap() }).await;
    drop(value);
    peak
}

#[tokio::test]
async fn the_estimate_is_above_the_peak_of_reading_arrays_too() {
    let array_chain = |depth: usize| (0..depth).fold(json!(0), |inner, _| json!([inner]));
    let mut shapes = shapes();
    shapes.push((
        "arrays of one element",
        Value::Array((0..49_000).map(|_| json!([0])).collect()),
    ));
    shapes.push((
        "chains of arrays, depth 30",
        Value::Array((0..3_000).map(|_| array_chain(30)).collect()),
    ));
    shapes.push(("text with escapes", json!("line\n".repeat(1_200_000))));
    shapes.push((
        "text of two byte characters",
        json!("\u{e9}".repeat(3_000_000)),
    ));
    shapes.push(("a key with escapes", json!({"k\n".repeat(1_000_000): 1})));
    shapes.push((
        "mixed",
        Value::Array(
            (0..5_000)
                .map(|i| json!({"a": [i, [i]], "b": "x"}))
                .collect(),
        ),
    ));
    for (what, value) in shapes {
        let text = value.to_string();
        let (weight, peak) = (memory_weight(&value), peak_of_reading(&text).await);
        // The text is read from the input, and an escape is decoded in a scratch buffer of about the
        // size of the string: the peak is the estimate plus the input at the highest.
        assert!(
            weight + text.len() >= peak,
            "{what}: estimated {weight} and {} bytes of input, reading it took {peak}",
            text.len()
        );
    }
}
