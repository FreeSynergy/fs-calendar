//! How much memory a JSON value needs.
//!
//! Pattern: none (a pure function; no pattern of coding.md section 2 fits a measure) — a node of
//! `serde_json::Value` takes 32 bytes however small it is, while a number in text takes two. The limits
//! of the calendar count this size, not the length of the text, or an event of millions of small nodes
//! passes a budget of bytes and fills the memory.

use serde_json::Value;

/// Bytes a node takes in memory, not counting what it points to.
const NODE_BYTES: usize = 32;

/// An estimate of the memory the value needs: 32 bytes a node, plus the text of strings and keys.
pub fn memory_weight(value: &Value) -> usize {
    let mut total = 0usize;
    let mut stack = vec![value];
    while let Some(node) = stack.pop() {
        total = total.saturating_add(NODE_BYTES);
        match node {
            Value::String(text) => total = total.saturating_add(text.len()),
            Value::Array(items) => stack.extend(items),
            Value::Object(members) => {
                for (key, member) in members {
                    total = total.saturating_add(NODE_BYTES + key.len());
                    stack.push(member);
                }
            }
            _ => {}
        }
    }
    total
}
