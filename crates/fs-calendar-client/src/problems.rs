//! The problems of the server as the errors of the core.
//!
//! Pattern: none — pure functions from a problem to an error. The table is the inverse of `errors.rs`
//! of `fs-calendar-server`: each RFC 9457 type becomes the variant of `fs_calendar_core::Error` it
//! came from, so the frontend's own reading of an error needs no change. The decision is by the
//! type, never by the words; what the variant carries beyond that is read from the text only behind
//! `read_detail` (`detail.rs`), the rest comes from the call ([`Subject`]). The generic readers are
//! those of `fs_interfaces::rpc::client::problems`.

use fs_calendar_core::Error;
use fs_interfaces::Unsupported;
use fs_interfaces::rpc::Problem;
use fs_interfaces::rpc::client::problems::{Failure, local_type, unsupported_of};
use fs_store::{Error as StoreError, ObjectId};

use crate::detail::read_detail;

/// What a call was about, for the variants that name it.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Subject {
    /// Nothing in particular (a list, an import).
    None,
    /// This event.
    Event(ObjectId),
}

/// A store error with the text of the server.
pub(crate) fn backend(text: String) -> Error {
    Error::Store(StoreError::Backend(text))
}

/// The error of the core for what the server (or the wire) said.
pub(crate) fn error_of(e: &capnp::Error, subject: &Subject) -> Error {
    match Failure::of(e) {
        Failure::Problem(problem) => from_problem(&problem, *subject),
        Failure::Unimplemented => Unsupported::new(
            "calendar",
            "the server does not know this method of the contract",
        )
        .into(),
        Failure::Other(text) => backend(format!("calendar server: {text}")),
    }
}

pub(crate) fn from_problem(p: &Problem, subject: Subject) -> Error {
    let kind = local_type(p);
    if kind == "unsupported" {
        return unsupported_of(&p.detail, "calendar").into();
    }
    read_detail(kind, &p.detail, subject).unwrap_or_else(|| match kind {
        "invalid-request" => Error::Store(StoreError::Invalid(p.detail.clone())),
        _ => backend(format!("calendar server ({}): {}", p.status, p.detail)),
    })
}
