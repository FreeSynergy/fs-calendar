//! The default [`RecurrenceEngine`], backed by the `rrule` crate (RFC 5545 semantics).
//!
//! Pattern: Adapter + Strategy — `rrule` is built on `chrono-tz` (tzdb 2025b), which we
//! deliberately do **not** use: the rule runs on wall-clock time labelled as UTC (so it never
//! consults a tz database) and the caller converts every local start with our own zone afterwards.

use chrono::{DateTime, NaiveDateTime, Utc};
use rrule::RRuleSet;

use super::engine::RecurrenceEngine;
use super::rrule_string::rrule_string;
use crate::error::{Error, Result};
use crate::model::RecurrenceRule;
use crate::time::Zone;

/// Recurrence through the `rrule` crate.
#[derive(Clone, Copy, Debug, Default)]
pub struct RRuleEngine;

impl RecurrenceEngine for RRuleEngine {
    fn starts(
        &self,
        rule: &RecurrenceRule,
        start: NaiveDateTime,
        zone: &Zone,
        after: DateTime<Utc>,
        before: DateTime<Utc>,
    ) -> Result<Vec<NaiveDateTime>> {
        let rr = rrule_string(rule, zone)?;
        // Local wall-clock time labelled UTC: no DST logic inside the rule engine.
        let dtstart = format!("DTSTART:{}Z", start.format("%Y%m%dT%H%M%S"));
        let set: RRuleSet = format!("{dtstart}\nRRULE:{rr}")
            .parse()
            .map_err(|err| Error::Recurrence(format!("{err}")))?;
        // The bounds are generous (callers add a margin of days), so converting them to
        // wall-clock is safe.
        let wall = |t: DateTime<Utc>| zone.to_local(t).and_utc().with_timezone(&rrule::Tz::UTC);
        let res = set.after(wall(after)).before(wall(before)).all(u16::MAX);
        if res.limited {
            return Err(Error::Recurrence(format!(
                "more than {} starts in the range: ask for a shorter range",
                u16::MAX
            )));
        }
        Ok(res
            .dates
            .iter()
            .map(chrono::DateTime::naive_local)
            .collect())
    }
}
