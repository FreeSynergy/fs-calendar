//! The problems of the server as the error of the calendar.
//!
//! Pattern: Adapter — an RPC error becomes a [`TasksError`] with the text the server gave. One kind
//! is told apart: a task the server calls invalid, because an import loses that task only.

use fs_calendar_core::TasksError;
use fs_interfaces::rpc::client::problems::{Failure, local_type};

/// What the server said about one task it did not store.
pub(crate) enum Said {
    /// Invalid (`tasks/invalid`, 422) or too large (`too-large`, 413): this task is refused, the call is fine.
    Refused(String),
    /// A task with this `uid` or id exists (`tasks/conflict`, 409): it is not overwritten.
    Exists,
}

/// Sort out the answer to a `create`; `None` is a failure of the call itself.
pub(crate) fn said(e: &capnp::Error) -> Option<Said> {
    match Failure::of(e) {
        Failure::Problem(p) => match local_type(&p) {
            "tasks/invalid" | "too-large" => Some(Said::Refused(p.detail)),
            // `Exists` of the task program names the task (`id`); any other conflict (the id is an object of
            // another program in a shared store) does not: that task is refused, not "already known".
            "tasks/conflict" if p.ext("id").is_some() => Some(Said::Exists),
            "tasks/conflict" => Some(Said::Refused(p.detail)),
            _ => None,
        },
        _ => None,
    }
}

/// The error of the calendar for what the server (or the wire) said.
pub(crate) fn error_of(e: &capnp::Error) -> TasksError {
    match Failure::of(e) {
        Failure::Problem(p) => {
            TasksError::new(format!("tasks server ({}): {}", p.status, p.detail))
        }
        Failure::Unimplemented => {
            TasksError::new("tasks server: it does not know this method of the contract")
        }
        Failure::Other(text) => TasksError::new(format!("tasks server: {text}")),
    }
}

/// The error of the calendar for an answer that could not be read.
pub(crate) fn unreadable(e: impl std::fmt::Display) -> TasksError {
    TasksError::new(format!("tasks server: unreadable answer: {e}"))
}
