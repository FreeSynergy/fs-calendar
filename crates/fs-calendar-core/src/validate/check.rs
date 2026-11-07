//! Entry points of the validation: `issues`, `validate`, `validate_event`.
//!
//! Pattern: Specification (composite) — event rules here, the shared ones from `fs-jscalendar`;
//! `validate` turns a non-empty list into `Error::Invalid`.

use chrono::Datelike;
use fs_jscalendar::validate::{Ctx, common, into_result, shared};
use fs_jscalendar::{Issue, quoted};

use super::overrides::check_overrides;
use crate::error::Result;
use crate::limits::{FIRST_YEAR, LAST_YEAR};
use crate::model::Event;
use crate::time::{Dur, Zone, parse_local};

/// All problems found in the event; empty = valid.
pub fn issues(e: &Event) -> Vec<Issue> {
    let mut c = Ctx::new();
    common(
        &mut c,
        e.type_.as_deref(),
        Event::TYPE,
        e.version.as_deref(),
        &e.uid,
    );
    match e.start.as_deref().map(parse_local) {
        None => c.bad("/start", "required for an Event"),
        Some(Ok(start)) if !(FIRST_YEAR..=LAST_YEAR).contains(&start.year()) => c.bad(
            "/start",
            format!("the year must be from {FIRST_YEAR} to {LAST_YEAR}, so the time cannot move at the edge of the calendar"),
        ),
        Some(_) => {} // a start that does not read is said by the shared rules
    }
    c.one_of(
        "/status",
        e.status.as_deref(),
        &["confirmed", "tentative", "cancelled"],
    );
    if let Some(d) = &e.duration {
        if Dur::parse(d).is_err() {
            c.bad("/duration", format!("not a duration: {}", quoted(d)));
        }
    }
    if let Some(z) = &e.end_time_zone {
        if Zone::parse(z).is_err() {
            c.bad(
                "/endTimeZone",
                format!("not an IANA time zone: {}", quoted(z)),
            );
        }
    }
    if let Some(r) = &e.recurrence_id {
        if parse_local(r).is_err() {
            c.bad(
                "/recurrenceId",
                format!("not a local date-time: {}", quoted(r)),
            );
        }
    }
    if let Some(z) = &e.recurrence_id_time_zone {
        if Zone::parse(z).is_err() {
            c.bad(
                "/recurrenceIdTimeZone",
                format!("not an IANA time zone: {}", quoted(z)),
            );
        }
    }
    shared(&mut c, e);
    check_overrides(&mut c, e);
    c.into_issues()
}

/// Validate; `Err(Error::Invalid)` lists every problem.
pub fn validate(e: &Event) -> Result<()> {
    Ok(into_result(issues(e))?)
}

/// Validate an event (same as [`validate`]).
pub fn validate_event(e: &Event) -> Result<()> {
    validate(e)
}
