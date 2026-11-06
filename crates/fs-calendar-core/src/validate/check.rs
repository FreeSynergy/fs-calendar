//! Entry points of the validation: `issues`, `validate`, `validate_event`, `validate_task`.
//!
//! Pattern: Specification (composite) — type-specific rules here, shared rules in `shared`;
//! `validate` turns a non-empty list into `Error::Invalid`.

use super::ctx::Ctx;
use super::shared::shared;
use crate::error::{Error, Issue, Result};
use crate::model::{CalendarObject, Event, Task, VERSION};
use crate::time::{Dur, Zone, parse_local, parse_utc};

/// All problems found in the object; empty = valid.
pub fn issues(obj: &CalendarObject) -> Vec<Issue> {
    let mut c = Ctx(vec![]);
    match obj {
        CalendarObject::Event(e) => {
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
        }
        CalendarObject::Task(t) => {
            common(
                &mut c,
                t.type_.as_deref(),
                "Task",
                t.version.as_deref(),
                &t.uid,
            );
            c.one_of(
                "/progress",
                t.progress.as_deref(),
                &[
                    "needs-action",
                    "in-process",
                    "completed",
                    "failed",
                    "cancelled",
                ],
            );
            if t.percent_complete.is_some_and(|p| p > 100) {
                c.bad("/percentComplete", "0..=100");
            }
            if let Some(s) = &t.due {
                if parse_local(s).is_err() {
                    c.bad("/due", format!("not a LocalDateTime: {s:?}"));
                }
            }
            if let Some(d) = &t.estimated_duration {
                if Dur::parse(d).is_err() {
                    c.bad("/estimatedDuration", format!("not a duration: {d:?}"));
                }
            }
            if let Some(s) = &t.progress_updated {
                if parse_utc(s).is_err() {
                    c.bad("/progressUpdated", "not a UTCDateTime");
                }
            }
            shared(&mut c, t);
        }
    }
    c.0
}

/// Validate; `Err(Error::Invalid)` lists every problem.
pub fn validate(obj: &CalendarObject) -> Result<()> {
    let i = issues(obj);
    if i.is_empty() {
        Ok(())
    } else {
        Err(Error::Invalid(i))
    }
}

/// Validate an event.
pub fn validate_event(e: &Event) -> Result<()> {
    validate(&CalendarObject::Event(e.clone()))
}

/// Validate a task.
pub fn validate_task(t: &Task) -> Result<()> {
    validate(&CalendarObject::Task(t.clone()))
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
