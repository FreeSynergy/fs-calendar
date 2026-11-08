//! The problems of the server as the error of the calendar.
//!
//! Pattern: Adapter — an RPC error becomes a [`TasksError`] with the text the server gave. One kind
//! is told apart: a task the server calls invalid, because an import loses that task only.

use fs_calendar_core::TasksError;
use fs_interfaces::rpc::client::problems::{Failure, local_type};

/// The server called what was sent invalid (`tasks/invalid`, 422): the task is refused, the call is fine.
pub(crate) fn refused(e: &capnp::Error) -> Option<String> {
    match Failure::of(e) {
        Failure::Problem(p) if local_type(&p) == "tasks/invalid" => Some(p.detail),
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
