//! The `uid`s of the stored events, read once for an import.
//!
//! Pattern: Value object — a set of normalised `uid`s with the two things an import asks of it. Why it
//! exists: the store answers "is this `uid` taken" by looking at every event of the type (a lookup by
//! an index field is an `EXISTS` for each row), so asking for each part of an import costs the square
//! of the count (1 000 lookups over 1 000 events took 1.1 s, 22 µs per stored event and lookup, in a
//! debug build). The set is read in pages with the cheapest query there is (no condition), once, and
//! kept up to date by the import itself, which holds the write lock (`writes`).

use std::collections::HashSet;

use fs_objects::{Order, Query};

use super::consts::{CONTEXT, EVENT_TYPE};
use super::handle::CalendarRepo;
use crate::error::Result;
use crate::uid::normalise_uid;

/// Events read per query while the set is built; the memory held at once is this many events.
const PAGE: usize = 500;

/// The `uid`s that are stored, and those an import has stored since, in the spelling `normalise_uid`
/// gives (any spelling of the same identity finds it).
#[derive(Debug, Default)]
pub(super) struct KnownUids(HashSet<String>);

impl KnownUids {
    /// Is an event with this `uid` stored, in any spelling of it?
    pub(super) fn contains(&self, uid: &str) -> bool {
        self.0.contains(&normalise_uid(uid))
    }

    /// An event with this `uid` was stored.
    pub(super) fn add(&mut self, uid: &str) {
        self.0.insert(normalise_uid(uid));
    }
}

impl CalendarRepo {
    /// The `uid` of every stored event, read in pages of [`PAGE`] (oldest first, so a page does not
    /// move while nothing is written; the caller holds the write lock).
    pub(super) async fn known_uids(&self) -> Result<KnownUids> {
        let mut known = KnownUids::default();
        let mut offset = 0;
        loop {
            let page = self
                .queries
                .query(
                    &Query::new()
                        .of_type(EVENT_TYPE)
                        .in_context(CONTEXT)
                        .ordered(Order::UpdatedAsc)
                        .limit(PAGE)
                        .offset(offset),
                )
                .await?;
            let read = page.len();
            for object in page {
                if let Some(uid) = object
                    .content
                    .get("uid")
                    .and_then(serde_json::Value::as_str)
                {
                    known.add(uid);
                }
            }
            if read < PAGE {
                return Ok(known);
            }
            offset += read;
        }
    }
}
