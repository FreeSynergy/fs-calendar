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
use crate::limits::MAX_RULE_STEPS;
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
        let (after, before) = (wall(after), wall(before));
        // A hard budget: the engine is stopped after `MAX_RULE_STEPS` dates, found or not, so a rule
        // whose cost the estimate (`recur::steps`) missed still cannot hold the caller.
        let (mut dates, mut steps) = (Vec::new(), 0u64);
        for date in &set {
            if date > before {
                break;
            }
            steps += 1;
            if steps > MAX_RULE_STEPS {
                return Err(Error::Limit(format!(
                    "the rule needs more than {MAX_RULE_STEPS} steps before the end of the window: ask for a shorter range"
                )));
            }
            if date >= after {
                if dates.len() >= usize::from(u16::MAX) {
                    return Err(Error::Limit(format!(
                        "more than {} starts in the range: ask for a shorter range",
                        u16::MAX
                    )));
                }
                dates.push(date.naive_local());
            }
        }
        Ok(dates)
    }
}
