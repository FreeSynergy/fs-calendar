//! Errors of the core as RPC errors.
//!
//! Pattern: Adapter — each error the core can name becomes a typed RFC 9457 problem, so a consumer
//! can tell "not found" from "not possible" without reading a message.

use crate::facts::with_facts;
use fs_calendar_core::Error;
use fs_calendar_core::limits::MAX_EVENT_BYTES;
use fs_interfaces::rpc::{Problem, TOO_LARGE, failed, problem_keys, unsupported};

const NOT_FOUND: &str = "https://freesynergy.net/problems/calendar/not-found";
const CONFLICT: &str = "https://freesynergy.net/problems/calendar/conflict";
const INVALID: &str = "https://freesynergy.net/problems/calendar/invalid";
const TOO_EXPENSIVE: &str = "https://freesynergy.net/problems/calendar/rule-too-expensive";
const ENCRYPTION_REQUIRED: &str = "https://freesynergy.net/problems/events/encryption-required";

/// A core error as an RPC error. `Json` means data already stored no longer reads: not the caller's
/// fault. What a client sends is read before the core sees it (`wire::read_event`).
// By value so that it can be handed to `map_err` as it is.
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rpc_error(e: Error) -> capnp::Error {
    use fs_store::Error as Store;
    let text = e.to_string();
    let typed = |kind: &str, title: &str, status: u16| {
        with_facts(Problem::new(kind, title, text.clone(), status), &e).into_error()
    };
    match &e {
        Error::Unsupported(not_possible) => unsupported(not_possible),
        Error::NotFound(_) | Error::Store(Store::NotFound(_)) => {
            typed(NOT_FOUND, "Event not found", 404)
        }
        Error::Conflict(_) | Error::Changed { .. } | Error::Store(Store::Conflict(_)) => {
            typed(CONFLICT, "Conflict", 409)
        }
        Error::Invalid(_)
        | Error::Time(_)
        | Error::Recurrence(_)
        | Error::ICalendar(_)
        | Error::MediaType(_)
        | Error::Store(Store::Invalid(_)) => typed(INVALID, "Invalid event", 422),
        Error::RuleTooExpensive(_) => typed(TOO_EXPENSIVE, "The rule is too expensive", 422),
        Error::Store(Store::EncryptionRequired(_)) => {
            typed(ENCRYPTION_REQUIRED, "Encryption required", 409)
        }
        Error::TooLarge(size) => with_facts(
            Problem::too_large(text, *size as u64, MAX_EVENT_BYTES as u64),
            &e,
        )
        .into_error(),
        Error::TooHeavy(_) | Error::Limit(_) => {
            with_facts(Problem::new(TOO_LARGE, "Too large", text.clone(), 413), &e).into_error()
        }
        Error::Json(_) | Error::Tasks(_) | Error::Stored(_) | Error::Store(_) => failed(text),
    }
}

/// The request itself is not acceptable.
pub(crate) fn invalid(detail: impl Into<String>) -> capnp::Error {
    Problem::new(INVALID, "Invalid event", detail, 422).into_error()
}

/// The request holds a value that does not read as an event: the detail text as the core says it, and
/// the facts of it as extensions.
pub(crate) fn invalid_event(e: &Error) -> capnp::Error {
    with_facts(
        Problem::new(INVALID, "Invalid event", e.to_string(), 422),
        e,
    )
    .into_error()
}

/// The request is not JSON the event limits can read: the reason is the detail and the `issue`.
pub(crate) fn invalid_issue(reason: &str) -> capnp::Error {
    Problem::new(INVALID, "Invalid event", reason, 422)
        .with_ext(problem_keys::ISSUE, reason)
        .into_error()
}
