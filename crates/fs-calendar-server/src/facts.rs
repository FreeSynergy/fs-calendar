//! The facts behind a problem, as typed extensions.
//!
//! Pattern: none (a function that only computes from its argument). What a client used to cut out
//! of the detail text (`coding.md` §2a) is stated here as `id`, `kind` and `issue`; the detail text
//! stays as it was. The table is in `DESIGN.md`.

use fs_calendar_core::Error;
use fs_interfaces::rpc::{Problem, problem_keys as keys};
use fs_objects::Error as StoreError;

/// `kind` of an event.
pub(crate) const EVENT: &str = "event";
/// `kind` of a time, duration or time zone that could not be handled.
pub(crate) const TIME: &str = "time";
/// `kind` of a recurrence rule that could not be evaluated.
pub(crate) const RECURRENCE: &str = "recurrence";
/// `kind` of an iCalendar text that could not be read or written.
pub(crate) const ICALENDAR: &str = "icalendar";
/// `kind` of a media type no format is known for; the `issue` is the media type.
pub(crate) const MEDIA_TYPE: &str = "media-type";

/// `problem` with every fact the core error `e` knows.
pub(crate) fn with_facts(problem: Problem, e: &Error) -> Problem {
    match e {
        Error::NotFound(id) | Error::Changed { id, .. } => problem
            .with_ext(keys::ID, &id.to_string())
            .with_ext(keys::KIND, EVENT),
        Error::Store(StoreError::NotFound(id)) => problem.with_ext(keys::ID, &id.to_string()),
        Error::Invalid(issues) => issues.iter().fold(problem, |p, issue| {
            p.with_ext(keys::ISSUE, &issue.to_string())
        }),
        Error::Time(reason) => problem
            .with_ext(keys::KIND, TIME)
            .with_ext(keys::ISSUE, reason),
        Error::Recurrence(reason) => problem
            .with_ext(keys::KIND, RECURRENCE)
            .with_ext(keys::ISSUE, reason),
        Error::ICalendar(reason) => problem
            .with_ext(keys::KIND, ICALENDAR)
            .with_ext(keys::ISSUE, reason),
        Error::MediaType(media) => problem
            .with_ext(keys::KIND, MEDIA_TYPE)
            .with_ext(keys::ISSUE, media),
        Error::RuleTooExpensive(reason) | Error::TooHeavy(reason) | Error::Limit(reason) => {
            problem.with_ext(keys::ISSUE, reason)
        }
        Error::Json(inner) => problem.with_ext(keys::ISSUE, &inner.to_string()),
        Error::Store(StoreError::Invalid(reason)) => problem.with_ext(keys::ISSUE, reason),
        _ => problem,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errors::rpc_error;

    fn problem(e: Error) -> Problem {
        Problem::from_error(&rpc_error(e)).expect("a problem")
    }

    #[test]
    fn a_time_a_rule_and_an_icalendar_error_say_what_they_are_about_without_the_text_prefix() {
        for (e, kind, reason) in [
            (Error::Time("no such zone".into()), TIME, "no such zone"),
            (Error::Recurrence("no end".into()), RECURRENCE, "no end"),
            (Error::ICalendar("no BEGIN".into()), ICALENDAR, "no BEGIN"),
        ] {
            let p = problem(e);
            assert_eq!(p.ext("kind"), Some(kind));
            assert_eq!(p.ext("issue"), Some(reason));
            assert!(p.detail.ends_with(reason) && p.detail.len() > reason.len());
        }
    }

    #[test]
    fn an_event_too_large_gives_its_size_and_the_limit() {
        let p = problem(Error::TooLarge(20_000_000));
        assert_eq!(p.status, 413);
        assert_eq!(p.ext_u64("size"), Some(20_000_000));
        assert_eq!(
            p.ext_u64("limit"),
            Some(fs_calendar_core::limits::MAX_EVENT_BYTES as u64)
        );
    }

    #[test]
    fn an_event_too_heavy_or_a_limit_gives_its_reason_as_an_issue() {
        for e in [
            Error::TooHeavy("1 value".into()),
            Error::Limit("1 value".into()),
        ] {
            let p = problem(e);
            assert_eq!((p.status, p.ext("issue")), (413, Some("1 value")));
        }
    }
}
