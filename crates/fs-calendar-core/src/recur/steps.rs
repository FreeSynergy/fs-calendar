//! What a rule costs: starts per period, and how far a window lies from the series start.
//!
//! Pattern: Specification — pure predicates over a rule (is a period too dense, a walk too long),
//! and the pure shift of its start. `rrule` walks from
//! `DTSTART`, so the cost of a question grows with the distance to the series start, and one period
//! of a rule with `BY` parts can hold millions of starts. Both are estimated from the rule alone, an
//! upper bound, before the engine starts; the engine also counts its steps (`RRuleEngine`).

use chrono::NaiveDateTime;

use crate::error::{Error, Result};
use crate::limits::{MAX_RULE_STEPS, MAX_STARTS_PER_PERIOD, MAX_UNBOUNDED_STARTS_PER_DAY};
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
    if rule.count.is_none() && rule.until.is_none() {
        let step = period_secs(&rule.frequency)
            .saturating_mul(i64::from(rule.interval.unwrap_or(1).max(1)))
            .max(1);
        let per_day = u128::from(per) * 86_400 / u128::from(step.unsigned_abs());
        if per_day > u128::from(MAX_UNBOUNDED_STARTS_PER_DAY) {
            return Err(Error::RuleTooExpensive(format!(
                "a rule without an end gives up to {per_day} starts a day, at most {MAX_UNBOUNDED_STARTS_PER_DAY} are possible; give it a count or an until"
            )));
        }
    }
    if per > MAX_STARTS_PER_PERIOD {
        return Err(Error::RuleTooExpensive(format!(
            "one {} gives up to {per} starts, at most {MAX_STARTS_PER_PERIOD} are possible",
            rule.frequency
        )));
    }
    Ok(())
}

/// The start moved forward by whole periods of the rule, to the last one not after `after`, where that
/// gives the same instances as walking from the real start; otherwise the start itself.
///
/// This holds for `daily` and `weekly` rules (a shift by whole `interval` periods keeps every `BY`
/// part in place), and for `hourly`, `minutely` and `secondly` rules **without any `BY` part**, whose
/// instances are exactly `start + n * interval * period`. It does not hold with `bySetPosition` (the
/// set of a period depends on where the walk begins), with a `count` (it counts from the real start),
/// or for a sub-daily rule with `BY` filters (the engine aligns them to the day, not to the start); the
/// tests compare with the engine from the real start over many rules (`tests/jump.rs`). Never behind
/// the `until` of the rule. The real start stays an instance of its own (the caller adds it).
pub(super) fn skip_to(
    rule: &RecurrenceRule,
    start: NaiveDateTime,
    after: NaiveDateTime,
) -> NaiveDateTime {
    let by_parts = !(rule.by_month.is_empty()
        && rule.by_month_day.is_empty()
        && rule.by_day.is_empty()
        && rule.by_year_day.is_empty()
        && rule.by_week_no.is_empty()
        && rule.by_hour.is_empty()
        && rule.by_minute.is_empty()
        && rule.by_second.is_empty());
    let allowed = rule.count.is_none()
        && rule.by_set_position.is_empty()
        && match rule.frequency.as_str() {
            "daily" | "weekly" => true,
            "hourly" | "minutely" | "secondly" => !by_parts,
            _ => false,
        };
    if !allowed {
        return start;
    }
    let step =
        period_secs(&rule.frequency).saturating_mul(i64::from(rule.interval.unwrap_or(1).max(1)));
    let whole = (after - start).num_seconds() / step.max(1);
    if whole <= 0 {
        return start;
    }
    let moved = chrono::Duration::try_seconds(whole.saturating_mul(step))
        .and_then(|by| start.checked_add_signed(by))
        .unwrap_or(start);
    match rule.until.as_deref().map(parse_local) {
        Some(Ok(until)) if moved > until => start,
        _ => moved,
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
    let periods = u64::try_from(span / step.max(1))
        .unwrap_or(0)
        .saturating_add(1);
    let mut steps = periods.saturating_mul(starts_per_period(rule));
    if let Some(count) = rule.count {
        steps = steps.min(u64::from(count));
    }
    if steps > MAX_RULE_STEPS {
        return Err(Error::Recurrence(format!(
            "the window lies too far from the series start: up to {steps} starts of the rule, at most {MAX_RULE_STEPS} are walked"
        )));
    }
    Ok(())
}
