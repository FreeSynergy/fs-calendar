//! The running count of a bounded read: values, estimated memory, and the first limit passed.
//!
//! Pattern: Value object — plain counters with the limits they are held to.

use std::cell::Cell;

use super::bounded::Bounds;
use super::weight::{ENTRY_BYTES, LEAF_ENTRIES, OBJECT_BYTES};

pub(super) const MARK: &str = "past the limits: ";

pub(super) struct Counter {
    pub(super) bounds: Bounds,
    pub(super) nodes: Cell<usize>,
    pub(super) memory: Cell<usize>,
}

impl Counter {
    pub(super) fn charge<E: serde::de::Error>(&self, bytes: usize) -> std::result::Result<(), E> {
        let (nodes, memory) = (
            self.nodes.get() + 1,
            self.memory.get().saturating_add(bytes),
        );
        self.nodes.set(nodes);
        self.memory.set(memory);
        if nodes > self.bounds.nodes {
            return Err(E::custom(format!(
                "{MARK}more than {} values",
                self.bounds.nodes
            )));
        }
        if memory > self.bounds.memory {
            return Err(E::custom(format!(
                "{MARK}more than {} bytes of memory",
                self.bounds.memory
            )));
        }
        Ok(())
    }

    /// The `n`th entry (from 1) of an object: its key, and a new leaf at every fifth.
    pub(super) fn entry<E: serde::de::Error>(
        &self,
        key: &str,
        n: usize,
    ) -> std::result::Result<(), E> {
        let leaf = if n > 1 && (n - 1) % LEAF_ENTRIES == 0 {
            OBJECT_BYTES
        } else {
            0
        };
        self.memory.set(
            self.memory
                .get()
                .saturating_add(ENTRY_BYTES + key.len() + leaf),
        );
        if self.memory.get() > self.bounds.memory {
            return Err(E::custom(format!(
                "{MARK}more than {} bytes of memory",
                self.bounds.memory
            )));
        }
        Ok(())
    }
}
