//! Error type of the calendar core.
//!
//! Pattern: none (plain error enum via `thiserror`) — one `Error` for every public operation,
//! with `From` conversions for the store and JSON layers.

use fs_jscalendar::Issue;
use fs_store::ObjectId;

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
    /// The task interface of fs-tasks failed.
    #[error("tasks: {0}")]
    Tasks(#[from] fs_tasks_core::Error),
    /// JSON could not be read or written.
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    /// No import / export format is known for this media type.
    #[error("unknown media type {0:?} (known: text/calendar, application/jscalendar+json)")]
    MediaType(String),
    /// The explicit "not possible" of a provider (ADR 0045).
    #[error(transparent)]
    Unsupported(#[from] fs_interfaces::Unsupported),
    /// The operation conflicts with the current state (duplicate uid, cancelled event, ...).
    #[error("{0}")]
    Conflict(String),
}

impl From<fs_jscalendar::Error> for Error {
    fn from(e: fs_jscalendar::Error) -> Self {
        match e {
            fs_jscalendar::Error::Invalid(issues) => Self::Invalid(issues),
            fs_jscalendar::Error::Time(m) => Self::Time(m),
            fs_jscalendar::Error::ICalendar(m) => Self::ICalendar(m),
            fs_jscalendar::Error::Json(e) => Self::Json(e),
        }
    }
}
