//! The revision of a stored event, for compare-and-set.
//!
//! Pattern: Newtype — a number that changes whenever the stored event does (`Event.revision` of the
//! contract: compare for equality only, it is no counter). A writer hands back the revision it read;
//! [`Calendar::update_checked`](crate::Calendar::update_checked) writes only if the event is still at it.

use std::fmt;

use chrono::{DateTime, Utc};
use fs_store::StoredObject;

/// What an event was when it was read: a fingerprint (FNV-1a) of the store's `updated` (microseconds)
/// and the JSON of the content, so two writes within one tick still differ.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Revision(u64);

impl Revision {
    /// The revision of the object as stored now.
    pub fn of(object: &StoredObject) -> Self {
        Self::of_text(object.envelope.updated, &object.content.to_string())
    }

    /// The same from the parts, for a caller that already holds the text of the content.
    pub fn of_text(updated: DateTime<Utc>, content: &str) -> Self {
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        let bytes = updated.timestamp_micros().to_le_bytes();
        for byte in bytes.iter().chain(content.as_bytes()) {
            hash = (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3);
        }
        Self(hash)
    }

    /// The revision as a number, to hand over a process boundary.
    pub fn as_u64(self) -> u64 {
        self.0
    }
}

impl fmt::Display for Revision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:016x}", self.0)
    }
}

impl From<u64> for Revision {
    fn from(number: u64) -> Self {
        Self(number)
    }
}
