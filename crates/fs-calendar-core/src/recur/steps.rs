//! How far a window lies from the start of a series, in steps of its rule.
//!
//! Pattern: Specification — `rrule` walks from `DTSTART`, so the cost of a question grows with the
//! distance to the series start, not with the size of the window. A question that would walk more than
//! [`MAX_RULE_STEPS`] is refused before the engine starts, instead of freezing every caller.

use chrono::NaiveDateTime;

use crate::error::{Error, Result};
use crate::limits::MAX_RULE_STEPS;
use crate::model::RecurrenceRule;
use crate::time::parse_local;

/// The shortest time between two starts of a rule of this frequency, in seconds.
fn period_secs(frequency: &str) -> i64 {
    match frequency {
        "minutely" => 60,
        "hourly" => 3600,
        "daily" => 86_400,
        "weekly" => 7 * 86_400,
        "monthly" => 28 * 86_400,
        "yearly" => 365 * 86_400,
        // `secondly`, and a name the engine will refuse anyway: the shortest period is the safe side.
        _ => 1,
    }
}

/// Refuse a question whose walk from `start` to `before` (both wall-clock) is too long.
pub(super) fn check_steps(
    rule: &RecurrenceRule,
    start: NaiveDateTime,
    before: NaiveDateTime,
) -> Result<()> {
    let end = match rule.until.as_deref().map(parse_local) {
        Some(Ok(until)) => until.min(before),
        _ => before,
    };
    let span = (end - start).num_seconds();
    let step =
        period_secs(&rule.frequency).saturating_mul(i64::from(rule.interval.unwrap_or(1).max(1)));
    let mut steps = u64::try_from(span / step.max(1)).unwrap_or(0);
    if let Some(count) = rule.count {
        steps = steps.min(u64::from(count));
    }
    if steps > MAX_RULE_STEPS {
        return Err(Error::Recurrence(format!(
            "the window lies too far from the series start: {steps} steps of the rule, at most {MAX_RULE_STEPS} are walked"
        )));
    }
    Ok(())
}
