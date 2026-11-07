//! Errors of the core as RPC errors.
//!
//! Pattern: Adapter — each error the core can name becomes a typed RFC 9457 problem, so a consumer
//! can tell "not found" from "not possible" without reading a message.

use fs_calendar_core::Error;
use fs_interfaces::rpc::{Problem, failed, too_large, unsupported};

const NOT_FOUND: &str = "https://freesynergy.net/problems/calendar/not-found";
const CONFLICT: &str = "https://freesynergy.net/problems/calendar/conflict";
const INVALID: &str = "https://freesynergy.net/problems/calendar/invalid";
const TOO_EXPENSIVE: &str = "https://freesynergy.net/problems/calendar/rule-too-expensive";
const ENCRYPTION_REQUIRED: &str = "https://freesynergy.net/problems/events/encryption-required";

/// A core error as an RPC error. `Json` means data already stored no longer reads: not the caller's
/// fault. What a client sends is read before the core sees it (`wire::read_event`).
pub(crate) fn rpc_error(e: Error) -> capnp::Error {
    use fs_store::Error as Store;
    let text = e.to_string();
    match e {
        Error::Unsupported(not_possible) => unsupported(&not_possible),
        Error::NotFound(_) | Error::Store(Store::NotFound(_)) => {
            Problem::new(NOT_FOUND, "Event not found", text, 404).into_error()
        }
        Error::Conflict(_) | Error::Store(Store::Conflict(_)) => {
            Problem::new(CONFLICT, "Conflict", text, 409).into_error()
        }
        Error::Invalid(_)
        | Error::Time(_)
        | Error::Recurrence(_)
        | Error::ICalendar(_)
        | Error::MediaType(_)
        | Error::Store(Store::Invalid(_)) => invalid(text),
        Error::RuleTooExpensive(_) => {
            Problem::new(TOO_EXPENSIVE, "The rule is too expensive", text, 422).into_error()
        }
        Error::Store(Store::EncryptionRequired(_)) => {
            Problem::new(ENCRYPTION_REQUIRED, "Encryption required", text, 409).into_error()
        }
        Error::TooLarge(_) | Error::TooHeavy(_) | Error::Limit(_) => too_large(text),
        Error::Json(_) | Error::Tasks(_) | Error::Store(_) => failed(text),
    }
}

/// The request itself is not acceptable.
pub(crate) fn invalid(detail: impl Into<String>) -> capnp::Error {
    Problem::new(INVALID, "Invalid event", detail, 422).into_error()
}
