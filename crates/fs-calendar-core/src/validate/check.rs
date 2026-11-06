//! Entry points of the validation: `issues`, `validate`, `validate_event`.
//!
//! Pattern: Specification (composite) — type-specific rules here, shared rules in `shared`;
//! `validate` turns a non-empty list into `Error::Invalid`.

use super::ctx::Ctx;
use super::shared::shared;
use crate::error::{Error, Issue, Result};
use crate::model::{Event, VERSION};
use crate::time::{Dur, Zone};

/// All problems found in the event; empty = valid.
pub fn issues(e: &Event) -> Vec<Issue> {
    let mut c = Ctx(vec![]);
    common(
        &mut c,
        e.type_.as_deref(),
        "Event",
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
    c.0
}

/// Validate; `Err(Error::Invalid)` lists every problem.
pub fn validate(e: &Event) -> Result<()> {
    let i = issues(e);
    if i.is_empty() {
        Ok(())
    } else {
        Err(Error::Invalid(i))
    }
}

/// Validate an event (same as [`validate`]).
pub fn validate_event(e: &Event) -> Result<()> {
    validate(e)
}

fn common(c: &mut Ctx, ty: Option<&str>, want: &str, version: Option<&str>, uid: &str) {
    if ty != Some(want) {
        c.bad("/@type", format!("must be {want:?}"));
    }
    if version != Some(VERSION) {
        c.bad(
            "/version",
            "required, must be \"2.0\" (upgrade RFC 8984 data with `upgrade`)",
        );
    }
    if uid.trim().is_empty() {
        c.bad("/uid", "required");
    }
}
