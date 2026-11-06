//! Rules for the recurrence rule.
//!
//! Pattern: Specification — frequency, interval, count / until and day names.

use super::ctx::{Ctx, DAYS, FREQ};
use crate::model::RecurrenceRule;
use crate::time::{parse_local, parse_utc};

pub(super) fn rule(c: &mut Ctx, r: &RecurrenceRule) {
    if !FREQ.contains(&r.frequency.as_str()) {
        c.bad(
            "/recurrenceRule/frequency",
            format!("unknown frequency {:?}", r.frequency),
        );
    }
    if r.interval == Some(0) {
        c.bad("/recurrenceRule/interval", "must be >= 1");
    }
    if r.count.is_some() && r.until.is_some() {
        c.bad(
            "/recurrenceRule",
            "`count` and `until` are mutually exclusive",
        );
    }
    if r.count == Some(0) {
        c.bad("/recurrenceRule/count", "must be >= 1");
    }
    if let Some(u) = &r.until {
        if parse_local(u).is_err() && parse_utc(u).is_err() {
            c.bad("/recurrenceRule/until", format!("not a date-time: {u:?}"));
        }
    }
    for d in &r.by_day {
        if !DAYS.contains(&d.day.as_str()) {
            c.bad("/recurrenceRule/byDay", format!("unknown day {:?}", d.day));
        }
    }
    if r.rscale
        .as_deref()
        .is_some_and(|s| !s.eq_ignore_ascii_case("gregorian"))
    {
        c.bad("/recurrenceRule/rscale", "only gregorian is supported");
    }
}
