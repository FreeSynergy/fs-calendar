//! The error of a task program, as the calendar sees it.
//!
//! Pattern: Newtype — only the text of what went wrong. The calendar cannot do anything about a task
//! program that fails except say so, and it must not name the error type of another program.

/// The task program could not be reached or refused the call; `0` says why.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct TasksError(pub String);

impl TasksError {
    /// An error with this text.
    pub fn new(why: impl Into<String>) -> Self {
        Self(why.into())
    }
}
