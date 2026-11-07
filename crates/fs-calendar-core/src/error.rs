//! Error type of the calendar core.
//!
//! Pattern: none (plain error enum via `thiserror`) — one `Error` for every public operation,
//! with `From` conversions for the store and JSON layers.

use fs_jscalendar::Issue;
use fs_store::ObjectId;

use crate::revision::Revision;

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
    /// The event would be too large to come back in one answer (`limits::MAX_EVENT_BYTES`).
    #[error("the event and its name have {0} bytes, at most {max} are possible", max = crate::limits::MAX_EVENT_BYTES)]
    TooLarge(usize),
    /// The recurrence rule would give more starts than the calendar can walk (`limits`).
    #[error("the rule is too expensive: {0}")]
    RuleTooExpensive(String),
    /// The event has too many values, levels or bytes of memory (many small values are far larger in
    /// memory than as text).
    #[error("the event is too heavy: {0}")]
    TooHeavy(String),
    /// A limit of the calendar stopped the question: the answer would be too large or too far to walk.
    #[error("{0}")]
    Limit(String),
    /// The event was changed since the caller read it (`update_checked` with an old revision);
    /// nothing was written. Pressing Save again does not help, reading the event again does.
    #[error("conflict: the event was changed by someone else (now at revision {now})")]
    Changed {
        /// The event.
        id: ObjectId,
        /// The revision it has now.
        now: Revision,
    },
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
