//! Error type of the calendar core.
//!
//! Pattern: none (plain error enum via `thiserror`) — one `Error` for every public operation,
//! with `From` conversions for the store and JSON layers.

use fs_store::ObjectId;

/// One validation problem with a path into the JSCalendar object.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Issue {
    /// JSON-pointer-like path, e.g. `/recurrenceRule/count`.
    pub path: String,
    /// What is wrong.
    pub message: String,
}

impl std::fmt::Display for Issue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.path, self.message)
    }
}

/// Result of every fallible operation of this crate.
pub type Result<T> = std::result::Result<T, Error>;

/// Everything that can go wrong in the calendar core.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The object store failed.
    #[error("object store: {0}")]
    Store(#[from] fs_store::Error),
    /// The object violates JSCalendar 2.0 (all problems listed).
    #[error("JSCalendar 2.0 violation: {}", .0.iter().map(ToString::to_string).collect::<Vec<_>>().join("; "))]
    Invalid(Vec<Issue>),
    /// No calendar object with this id.
    #[error("calendar object {0} not found")]
    NotFound(ObjectId),
    /// A date-time, duration or time zone could not be handled.
    #[error("time: {0}")]
    Time(String),
    /// A recurrence rule could not be evaluated.
    #[error("recurrence: {0}")]
    Recurrence(String),
    /// iCalendar text could not be read or written.
    #[error("iCalendar: {0}")]
    ICalendar(String),
    /// JSON could not be read or written.
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    /// The operation conflicts with the current state (duplicate uid, cancelled event, ...).
    #[error("{0}")]
    Conflict(String),
}
