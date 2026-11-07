//! The table of problem types: each type of the server and the variant of the core it becomes.
//!
//! Pattern: none — a test of `problems.rs`, a table of inputs and answers; no behaviour of its own.

use fs_calendar_core::Error;
use fs_interfaces::rpc::Problem;
use fs_objects::{Error as StoreError, ObjectId};

use crate::problems::{Subject, from_problem};

const BASE: &str = "https://freesynergy.net/problems/";

fn problem(kind: &str, detail: &str, status: u16) -> Problem {
    Problem::new(format!("{BASE}{kind}"), "t", detail, status)
}

fn error(kind: &str, detail: &str, subject: Subject) -> Error {
    from_problem(&problem(kind, detail, 400), subject)
}

#[test]
fn not_found_names_the_event_from_the_text_else_from_the_call() {
    let (named, asked) = (ObjectId::new(), ObjectId::new());
    let text = format!("calendar object {named} not found");
    assert!(
        matches!(error("calendar/not-found", &text, Subject::Event(asked)), Error::NotFound(id) if id == named)
    );
    assert!(
        matches!(error("calendar/not-found", "gone", Subject::Event(asked)), Error::NotFound(id) if id == asked)
    );
    assert!(matches!(
        error("calendar/not-found", "gone", Subject::None),
        Error::Store(StoreError::Backend(_))
    ));
}

#[test]
fn the_type_decides_and_the_words_only_fill() {
    // The same words under another type are not read as a change.
    let conflict = error("calendar/conflict", "conflict: moved", Subject::None);
    assert!(crate::changed_elsewhere(&conflict));
    let other = error(
        "calendar/conflict",
        "an object with id x exists",
        Subject::None,
    );
    assert!(!crate::changed_elsewhere(&other));
    let elsewhere = error("calendar/invalid", "conflict: moved", Subject::None);
    assert!(!matches!(elsewhere, Error::Conflict(_)));
}

#[test]
fn invalid_gives_back_the_issues_the_time_and_the_media_type() {
    match error(
        "calendar/invalid",
        "JSCalendar 2.0 violation: /status: bad; /start: \"a; b\" is no time",
        Subject::None,
    ) {
        Error::Invalid(issues) => {
            assert_eq!(issues.len(), 2, "{issues:?}");
            assert_eq!(
                (issues[0].path.as_str(), issues[1].path.as_str()),
                ("/status", "/start")
            );
            assert!(issues[1].message.contains("a; b"));
        }
        other => panic!("{other:?}"),
    }
    assert!(
        matches!(error("calendar/invalid", "time: no such zone", Subject::None), Error::Time(t) if t == "no such zone")
    );
    assert!(matches!(
        error("calendar/invalid", "recurrence: x", Subject::None),
        Error::Recurrence(_)
    ));
    assert!(matches!(
        error("calendar/invalid", "iCalendar: x", Subject::None),
        Error::ICalendar(_)
    ));
    assert!(matches!(
        error("calendar/invalid", "unknown media type \"text/plain\" (known: text/calendar, application/jscalendar+json)", Subject::None),
        Error::MediaType(t) if t == "text/plain"
    ));
    assert!(matches!(
        error("calendar/invalid", "something else", Subject::None),
        Error::Store(StoreError::Invalid(_))
    ));
}

#[test]
fn limits_keep_their_own_variants() {
    assert!(
        matches!(error("calendar/rule-too-expensive", "the rule is too expensive: 9 starts", Subject::None), Error::RuleTooExpensive(t) if t == "9 starts")
    );
    assert!(matches!(
        error(
            "too-large",
            "the event and its name have 20000000 bytes, at most 16777216 are possible",
            Subject::None
        ),
        Error::TooLarge(20_000_000)
    ));
    assert!(
        matches!(error("too-large", "the event is too heavy: 100001 values", Subject::None), Error::TooHeavy(t) if t == "100001 values")
    );
    assert!(matches!(
        error(
            "too-large",
            "the answer has 99 bytes; ask for a shorter window",
            Subject::None
        ),
        Error::Limit(_)
    ));
}

#[test]
fn encryption_required_names_the_event() {
    let id = ObjectId::new();
    assert!(
        matches!(error("events/encryption-required", "x", Subject::Event(id)), Error::Store(StoreError::EncryptionRequired(i)) if i == id)
    );
    assert!(matches!(
        error("events/encryption-required", "x", Subject::None),
        Error::Conflict(_)
    ));
}

#[test]
fn unsupported_and_the_rest() {
    assert!(matches!(
        error("unsupported", "event.update: not here", Subject::None),
        Error::Unsupported(_)
    ));
    assert!(matches!(
        error("invalid-request", "bad cursor", Subject::None),
        Error::Store(StoreError::Invalid(_))
    ));
    assert!(
        matches!(error("something-new", "?", Subject::None), Error::Store(StoreError::Backend(t)) if t.contains("400"))
    );
}
