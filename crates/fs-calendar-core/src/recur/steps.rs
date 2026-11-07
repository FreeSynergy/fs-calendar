//! What a rule costs: starts per period, and how far a window lies from the series start.
//!
//! Pattern: none (pure functions; no pattern of coding.md section 2 fits a bound). `rrule` walks from
//! `DTSTART`, so the cost of a question grows with the distance to the series start, and one period
//! of a rule with `BY` parts can hold millions of starts. Both are estimated from the rule alone, an
//! upper bound, before the engine starts; the engine also counts its steps (`RRuleEngine`).

use chrono::NaiveDateTime;

use crate::error::{Error, Result};
use crate::limits::{MAX_RULE_STEPS, MAX_STARTS_PER_PERIOD};
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

/// A list of `BY` values as a multiplier: an empty one does not expand, a long one at most `max`.
fn times(len: usize, max: u64) -> u64 {
    (len as u64).clamp(1, max)
}

/// The most starts one period of the rule can give: the days it can fall on times the times of a day.
/// An upper bound (`byDay` and `byMonthDay` may select fewer days than the period has).
pub(crate) fn starts_per_period(rule: &RecurrenceRule) -> u64 {
    let freq = rule.frequency.as_str();
    let selects_days = !(rule.by_month.is_empty()
        && rule.by_month_day.is_empty()
        && rule.by_day.is_empty()
        && rule.by_year_day.is_empty()
        && rule.by_week_no.is_empty());
    let days = match freq {
        "yearly" if selects_days => 366,
        "monthly" if selects_days => 31,
        "weekly" if selects_days => 7,
        _ => 1,
    };
    let hours = if matches!(freq, "hourly" | "minutely" | "secondly") {
        1
    } else {
        times(rule.by_hour.len(), 24)
    };
    let minutes = if matches!(freq, "minutely" | "secondly") {
        1
    } else {
        times(rule.by_minute.len(), 60)
    };
    let seconds = if freq == "secondly" {
        1
    } else {
        times(rule.by_second.len(), 60)
    };
    days * hours * minutes * seconds
}

/// Refuse a rule that gives more than [`MAX_STARTS_PER_PERIOD`] starts in one period. Called when an
/// event is written.
pub(crate) fn check_density(rule: &RecurrenceRule) -> Result<()> {
    let per = starts_per_period(rule);
    if per > MAX_STARTS_PER_PERIOD {
        return Err(Error::RuleTooExpensive(format!(
            "one {} gives up to {per} starts, at most {MAX_STARTS_PER_PERIOD} are possible",
            rule.frequency
        )));
    }
    Ok(())
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
    let periods = u64::try_from(span / step.max(1))
        .unwrap_or(0)
        .saturating_add(1);
    let mut steps = periods.saturating_mul(starts_per_period(rule));
    if let Some(count) = rule.count {
        steps = steps.min(u64::from(count));
    }
    if steps > MAX_RULE_STEPS {
        return Err(Error::Limit(format!(
            "the window lies too far from the series start: up to {steps} starts of the rule, at most {MAX_RULE_STEPS} are walked"
        )));
    }
    Ok(())
}
