//! Reading the words of a problem back into the fields of the core's errors.
//!
//! Pattern: none — pure functions (`read_detail`, `changed_elsewhere`) and the helpers behind them,
//! for one concept: the wire carries a type, a status and a text, and these errors need what only
//! the text holds (the issues of an invalid event, the size of a too large one, the `conflict:`
//! marker, the id in a not-found). All of it is here, behind these functions, so that the tag
//! `problem-ext` (which brings these as fields) replaces this file and nothing else. Which variant
//! comes out is decided by the type of the problem before; the words only fill it.

use fs_calendar_core::{Error, Issue};
use fs_interfaces::rpc::client::problems::{first_number, object_id_in, without_store_prefix};
use fs_objects::Error as StoreError;

use crate::problems::Subject;

/// What the contract says a refused `updateIf` starts its reason with.
const CHANGED: &str = "conflict:";

/// The error for a problem of `kind` from its `detail`; `None` for any other kind.
pub(crate) fn read_detail(kind: &str, detail: &str, subject: Subject) -> Option<Error> {
    Some(match kind {
        "calendar/not-found" => not_found(detail, subject),
        "calendar/conflict" => Error::Conflict(detail.to_string()),
        "calendar/invalid" => invalid(detail),
        "calendar/rule-too-expensive" => {
            Error::RuleTooExpensive(after(detail, "the rule is too expensive: "))
        }
        "events/encryption-required" => match (object_id_in(detail), subject) {
            (Some(id), _) | (None, Subject::Event(id)) => {
                Error::Store(StoreError::EncryptionRequired(id))
            }
            (None, Subject::None) => Error::Conflict(detail.to_string()),
        },
        "too-large" => too_large(detail),
        _ => return None,
    })
}

/// Is this the refusal of `update_checked`, "the event was changed by someone else"? The contract
/// marks that refusal with a reason that starts `conflict:`; the core has no variant of its own for
/// it yet, so this is the one place that reads the marker.
#[must_use]
pub fn changed_elsewhere(error: &Error) -> bool {
    matches!(error, Error::Conflict(text) if text.starts_with(CHANGED))
}

fn after(detail: &str, prefix: &str) -> String {
    detail.strip_prefix(prefix).unwrap_or(detail).to_string()
}

/// `calendar object <id> not found` names the event; if the text does not, the call does.
fn not_found(detail: &str, subject: Subject) -> Error {
    match (object_id_in(detail), subject) {
        (Some(id), _) | (None, Subject::Event(id)) => Error::NotFound(id),
        (None, Subject::None) => {
            crate::problems::backend(format!("calendar server: something is missing: {detail}"))
        }
    }
}

/// The texts the core's errors have, read back into their variants; any other text the server found
/// invalid stays a text.
fn invalid(detail: &str) -> Error {
    if let Some(list) = detail.strip_prefix("JSCalendar 2.0 violation: ") {
        return Error::Invalid(issues(list));
    }
    for (prefix, make) in [
        ("time: ", Error::Time as fn(String) -> Error),
        ("recurrence: ", Error::Recurrence),
        ("iCalendar: ", Error::ICalendar),
    ] {
        if let Some(what) = detail.strip_prefix(prefix) {
            return make(what.to_string());
        }
    }
    if let Some(rest) = detail.strip_prefix("unknown media type ") {
        let name = rest.split('"').nth(1).unwrap_or(rest);
        return Error::MediaType(name.to_string());
    }
    Error::Store(StoreError::Invalid(
        without_store_prefix(detail).to_string(),
    ))
}

/// `/path: message; /other: message`, cut at every `; ` outside a double-quoted value; a piece that
/// does not start with a path belongs to the message before it.
fn issues(list: &str) -> Vec<Issue> {
    let mut out: Vec<Issue> = Vec::new();
    for piece in pieces(list) {
        match piece.split_once(": ") {
            Some((path, message)) if path.starts_with('/') || out.is_empty() => {
                out.push(Issue {
                    path: path.to_string(),
                    message: message.to_string(),
                });
            }
            _ => {
                if let Some(last) = out.last_mut() {
                    last.message.push_str("; ");
                    last.message.push_str(piece);
                }
            }
        }
    }
    out
}

fn pieces(list: &str) -> Vec<&str> {
    let (mut out, mut start) = (Vec::new(), 0);
    let (mut quoted, mut escaped) = (false, false);
    for (at, c) in list.char_indices() {
        match c {
            _ if escaped => escaped = false,
            '\\' if quoted => escaped = true,
            '"' => quoted = !quoted,
            ';' if !quoted && list[at..].starts_with("; ") => {
                out.push(&list[start..at]);
                start = at + 2;
            }
            _ => {}
        }
    }
    out.push(&list[start..]);
    out
}

/// The size of a too large event (`the event and its name have N bytes, ...`), a too heavy one
/// (`the event is too heavy: ...`); anything else (an answer, a window) is a limit with the
/// server's words.
fn too_large(detail: &str) -> Error {
    if detail.starts_with("the event and its name have ") {
        return Error::TooLarge(first_number(detail));
    }
    if let Some(why) = detail.strip_prefix("the event is too heavy: ") {
        return Error::TooHeavy(why.to_string());
    }
    Error::Limit(detail.to_string())
}
