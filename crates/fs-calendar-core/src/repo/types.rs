//! Small data types of the repository API.
//!
//! Pattern: Value objects — plain data for query results and import reports.

use fs_store::ObjectId;

use crate::recur::Occurrence;

/// An event instance with the object it belongs to.
#[derive(Clone, Debug, PartialEq)]
pub struct Instance {
    /// The stored object the instance belongs to.
    pub object: ObjectId,
    /// The concrete instance.
    pub occurrence: Occurrence,
}

/// Result of an import.
#[derive(Clone, Debug, Default)]
pub struct ImportReport {
    /// Objects that were created.
    pub created: Vec<ObjectId>,
    /// `uid`s that already existed (not overwritten).
    pub skipped: Vec<String>,
    /// Entries that could not be converted or validated.
    pub failed: Vec<String>,
    /// What the 1.0 -> 2.0 upgrade changed.
    pub notes: Vec<String>,
}
