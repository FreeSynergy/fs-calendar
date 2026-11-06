//! Entry points of the validation: `issues`, `validate`, `validate_event`.
//!
//! Pattern: Specification (composite) — event rules here, the shared ones from `fs-jscalendar`;
//! `validate` turns a non-empty list into `Error::Invalid`.

use fs_jscalendar::Issue;
use fs_jscalendar::validate::{Ctx, common, into_result, shared};

use crate::error::Result;
use crate::model::Event;
use crate::time::{Dur, Zone};

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
    if e.start.is_none() {
        c.bad("/start", "required for an Event");
    }
    c.one_of(
        "/status",
        e.status.as_deref(),
        &["confirmed", "tentative", "cancelled"],
    );
    if let Some(d) = &e.duration {
        if Dur::parse(d).is_err() {
            c.bad("/duration", format!("not a duration: {d:?}"));
        }
    }
    if let Some(z) = &e.end_time_zone {
        if Zone::parse(z).is_err() {
            c.bad("/endTimeZone", format!("not an IANA time zone: {z:?}"));
        }
    }
    shared(&mut c, e);
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
