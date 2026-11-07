//! The facts behind a problem, as typed extensions.
//!
//! Pattern: none (a function that only computes from its argument). What a client used to cut out
//! of the detail text (`coding.md` §2a) is stated here as `id`, `kind` and `issue`; the detail text
//! stays as it was. The table is in `DESIGN.md`.

use fs_calendar_core::Error;
use fs_interfaces::rpc::{Problem, problem_keys as keys};
use fs_store::Error as StoreError;

/// `kind` of an event.
pub(crate) const EVENT: &str = "event";

/// `problem` with every fact the core error `e` knows.
pub(crate) fn with_facts(problem: Problem, e: &Error) -> Problem {
    match e {
        Error::NotFound(id) | Error::Changed { id, .. } => problem
            .with_ext(keys::ID, &id.to_string())
            .with_ext(keys::KIND, EVENT),
        Error::Store(StoreError::NotFound(id)) => problem.with_ext(keys::ID, &id.to_string()),
        _ => problem,
    }
}
