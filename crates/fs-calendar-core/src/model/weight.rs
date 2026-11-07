//! How much memory a JSON value needs, and how many nodes and levels it has.
//!
//! Pattern: Visitor — one walk over the nodes of a value, one arm per variant. A node of
//! `serde_json::Value` takes 32 bytes however small it is (a number is two bytes as text), and without
//! `preserve_order` an object is a `BTreeMap` whose first entry already takes a leaf of about 640
//! bytes. The limits of the calendar count this size, not the length of the text, or an event of
//! small objects passes a budget of bytes and fills the memory. The estimate is tested to be above
//! what a copy really takes (`tests/weight.rs`).

use serde_json::Value;

/// Bytes a node takes in memory, not counting what it points to.
pub const NODE_BYTES: usize = 32;
/// Bytes of one leaf of the tree of an object; an object has at least one.
pub const OBJECT_BYTES: usize = 640;
/// Entries a leaf of the tree of an object holds at least (so leaves are at most one per this many).
pub const LEAF_ENTRIES: usize = 5;
/// Bytes of an entry of an object beyond its key's text and its value: the key's `String`.
pub const ENTRY_BYTES: usize = 24;

/// The size of a value: estimated memory, number of nodes, deepest level.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Shape {
    /// Estimated bytes of memory.
    pub memory: usize,
    /// Number of values (every scalar, string, array, object and entry value counts one).
    pub nodes: usize,
    /// Levels of arrays and objects inside each other.
    pub depth: usize,
}

/// The shape of the value, counted without recursion.
pub fn shape(value: &Value) -> Shape {
    let mut shape = Shape::default();
    let mut stack = vec![(value, 1usize)];
    while let Some((node, depth)) = stack.pop() {
        shape.nodes += 1;
        shape.depth = shape.depth.max(depth);
        shape.memory = shape.memory.saturating_add(NODE_BYTES);
        match node {
            Value::String(text) => shape.memory = shape.memory.saturating_add(text.len()),
            Value::Array(items) => stack.extend(items.iter().map(|i| (i, depth + 1))),
            Value::Object(members) => {
                let leaves = members.len().div_ceil(LEAF_ENTRIES).max(1);
                shape.memory = shape.memory.saturating_add(leaves * OBJECT_BYTES);
                for (key, member) in members {
                    shape.memory = shape.memory.saturating_add(ENTRY_BYTES + key.len());
                    stack.push((member, depth + 1));
                }
            }
            _ => {}
        }
    }
    shape
}

/// An estimate of the memory the value needs.
pub fn memory_weight(value: &Value) -> usize {
    shape(value).memory
}
