//! Error type of the calendar core.

use fs_store::ObjectId;

/// One validation problem with a path into the JSCalendar object.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Issue {
    pub path: String,
    pub message: String,
}

impl std::fmt::Display for Issue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.path, self.message)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("object store: {0}")]
    Store(#[from] fs_store::Error),
    #[error("JSCalendar 2.0 violation: {}", .0.iter().map(|i| i.to_string()).collect::<Vec<_>>().join("; "))]
    Invalid(Vec<Issue>),
    #[error("calendar object {0} not found")]
    NotFound(ObjectId),
    #[error("time: {0}")]
    Time(String),
    #[error("recurrence: {0}")]
    Recurrence(String),
    #[error("iCalendar: {0}")]
    ICalendar(String),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Conflict(String),
}
