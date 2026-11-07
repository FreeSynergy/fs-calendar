//! The revision of a stored event, for compare-and-set.
//!
//! Pattern: Value object (Newtype) — a text that changes whenever the stored event does. A writer
//! hands back the revision it read; [`Calendar::update_checked`](crate::Calendar::update_checked)
//! writes only if the event is still at it.

use std::fmt;

use fs_store::StoredObject;

/// What an event was when it was read: the store's `updated` (microseconds) and a fingerprint of the
/// content (FNV-1a), so two writes within one tick still differ.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Revision(String);

impl Revision {
    /// The revision of the object as stored now.
    pub fn of(object: &StoredObject) -> Self {
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for byte in object.content.to_string().bytes() {
            hash = (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3);
        }
        Self(format!(
            "{}-{hash:016x}",
            object.envelope.updated.timestamp_micros()
        ))
    }

    /// The revision as text, to hand over a process boundary.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Revision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for Revision {
    fn from(text: &str) -> Self {
        Self(text.to_string())
    }
}
