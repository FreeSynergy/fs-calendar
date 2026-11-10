//! Core errors as the refusals the frontend knows.
//!
//! Pattern: Adapter — `fs_calendar_core::Error` to `BackendError`. A validation problem becomes `Rejected` with the
//! field of its JSON path, so the form marks it; everything else is `NotFound` or `Unavailable` with a text for the log.
//! Gaps (reported): `TypeProblem` has no general "invalid", so a problem of the core is shown as the closest kind; and
//! `BackendError` has no conflict, so a changed event is `Unavailable("conflict: ...")`.

use fs_admin_frontend::BackendError;
use fs_calendar_core::{Error, Issue};
use fs_types::{BasicType, TypeDescription};
use fs_types_frontend::{RecordIssue, RecordProblem, TypeProblem};

use super::record::Unreadable;

/// The field a JSON path (`/start`, `/recurrenceRule/count`) belongs to.
fn field_of(path: &str) -> String {
    path.trim_start_matches('/')
        .split('/')
        .next()
        .unwrap_or("")
        .to_owned()
}

fn problem_for(description: &TypeDescription, field: &str, message: &str) -> RecordProblem {
    if message.contains("required") {
        return RecordProblem::Missing;
    }
    match description.find_field(field).map(|f| f.basic) {
        Some(BasicType::Choice) => RecordProblem::Value(TypeProblem::NotInList),
        Some(BasicType::Link) => RecordProblem::Value(TypeProblem::BadUri),
        Some(BasicType::Place) => RecordProblem::Value(TypeProblem::BadPlace),
        Some(BasicType::Text)
            if matches!(field, "start" | "duration" | "timeZone" | "endTimeZone") =>
        {
            RecordProblem::Value(TypeProblem::BadDateTime)
        }
        _ => RecordProblem::Value(TypeProblem::Character),
    }
}

/// The findings for the issues of the core.
#[must_use]
pub fn issues(description: &TypeDescription, list: &[Issue]) -> Vec<RecordIssue> {
    list.iter()
        .map(|i| {
            let field = field_of(&i.path);
            RecordIssue {
                problem: problem_for(description, &field, &i.message),
                field,
                index: 0,
            }
        })
        .collect()
}

/// A refusal for a value that does not read.
#[must_use]
pub fn unreadable(u: Unreadable) -> BackendError {
    BackendError::Rejected(vec![RecordIssue {
        field: u.field,
        index: u.index,
        problem: RecordProblem::Value(TypeProblem::NotInList),
    }])
}

/// A refusal for an id that is not an event id.
#[must_use]
pub fn bad_id() -> BackendError {
    BackendError::Rejected(vec![RecordIssue {
        field: "id".into(),
        index: 0,
        problem: RecordProblem::Value(TypeProblem::BadUri),
    }])
}

/// A missing required input.
#[must_use]
pub fn missing(field: &str) -> BackendError {
    BackendError::Rejected(vec![RecordIssue {
        field: field.into(),
        index: 0,
        problem: RecordProblem::Missing,
    }])
}

/// The refusal for an error of the core.
#[must_use]
pub fn refusal(description: &TypeDescription, e: &Error) -> BackendError {
    match e {
        Error::Invalid(list) => BackendError::Rejected(issues(description, list)),
        Error::NotFound(_) => BackendError::NotFound,
        other => BackendError::Unavailable(other.to_string()),
    }
}
