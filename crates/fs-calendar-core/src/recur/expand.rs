//! Expansion of an event into its instances, and the end of a series.
//!
//! Pattern: Template method — the fixed skeleton (collect the rule's starts, add override keys, drop
//! exclusions, keep what overlaps the window, build only the instances that are returned) is here; the
//! rule itself is evaluated by the [`RecurrenceEngine`] strategy. A damaged override costs its own
//! instance: [`expand_lenient`] names it, [`expand_with`] is the strict form that fails on the first.

use chrono::{DateTime, Duration, Utc};

use super::candidates::{Candidate, Damage, back, candidates, forward};
use super::early::earliest_start;
use super::engine::RecurrenceEngine;
use super::instance::{instance, master_value};
use super::occurrence::Occurrence;
use super::rrule_engine::RRuleEngine;
use crate::error::{Error, Result};
use crate::model::Event;
use crate::time::Zone;

/// What an expansion found: the instances, what could not be built, and whether more were cut.
#[derive(Clone, Debug, Default)]
pub struct Expansion {
    /// Instances overlapping the window, ordered by start, at most the limit.
    pub items: Vec<Occurrence>,
    /// Instances left out because their override is damaged.
    pub damaged: Vec<Damage>,
    /// More instances overlapped the window than the limit lets through.
    pub cut: bool,
}

/// [`expand_with`] using the default `rrule` engine.
pub fn expand(
    e: &Event,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
    floating: &Zone,
    limit: usize,
) -> Result<Vec<Occurrence>> {
    expand_with(&RRuleEngine, e, from, to, floating, limit)
}

/// [`series_end_with`] using the default `rrule` engine.
pub fn series_end(e: &Event, floating: &Zone) -> Result<Option<DateTime<Utc>>> {
    series_end_with(&RRuleEngine, e, floating)
}

/// All instances of `e` that overlap `[from, to)`, ordered by start, at most `limit`. Fails on the
/// first damaged override; [`expand_lenient`] is the form that names it and goes on.
pub fn expand_with(
    engine: &dyn RecurrenceEngine,
    e: &Event,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
    floating: &Zone,
    limit: usize,
) -> Result<Vec<Occurrence>> {
    let found = expand_lenient(engine, e, from, to, floating, limit)?;
    match found.damaged.first() {
        Some(d) => Err(Error::Recurrence(format!(
            "override {}: {}",
            d.key, d.reason
        ))),
        None => Ok(found.items),
    }
}

/// The instances of `e` overlapping `[from, to)`, at most `limit`, and the overrides that could not be
/// built. Zero-length instances count when `from <= start < to`. Instances excluded by
/// `recurrenceOverrides` are omitted; patched ones come back patched; override keys that the rule
/// does not produce are extra instances (RDATE). Floating events are interpreted in `floating`.
/// An error is the event as a whole: its rule or its zone cannot be read. Only the `limit` instances
/// that are returned are built as events.
pub fn expand_lenient(
    engine: &dyn RecurrenceEngine,
    e: &Event,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
    floating: &Zone,
    limit: usize,
) -> Result<Expansion> {
    let (mut found, damaged) = candidates(engine, e, (from, to), floating)?;
    found.sort_by_cached_key(Candidate::order);
    let cut = found.len() > limit;
    found.truncate(limit);
    let master = master_value(e);
    let mut items = Vec::with_capacity(found.len());
    for candidate in found {
        match candidate {
            Candidate::Built(o) => items.push(*o),
            Candidate::Plain { local, .. } => {
                if let Some(o) = instance(&master, None, local, (from, to), floating)? {
                    items.push(o);
                }
            }
        }
    }
    Ok(Expansion {
        items,
        damaged,
        cut,
    })
}

/// End of the last instance (UTC), or `None` if the series is unbounded (no `count` / `until`)
/// or the rule cannot be evaluated. Non-recurring events return their own end.
pub fn series_end_with(
    engine: &dyn RecurrenceEngine,
    e: &Event,
    floating: &Zone,
) -> Result<Option<DateTime<Utc>>> {
    let first_end = super::occurrence::first_span(e, floating)?.1;
    let first_start = earliest_start(e, floating)?;
    let Some(rule) = &e.recurrence_rule else {
        return Ok(Some(extra_end(engine, e, floating, first_end)?));
    };
    if rule.count.is_none() && rule.until.is_none() {
        return Ok(None);
    }
    let far = forward(first_start, Duration::days(366 * 400));
    Ok(last_end(engine, e, (first_start, far), floating)?.or(Some(first_end)))
}

/// The latest end among the instances overlapping the window; strict about damaged overrides.
fn last_end(
    engine: &dyn RecurrenceEngine,
    e: &Event,
    window: (DateTime<Utc>, DateTime<Utc>),
    floating: &Zone,
) -> Result<Option<DateTime<Utc>>> {
    let (found, damaged) = candidates(engine, e, window, floating)?;
    if let Some(d) = damaged.first() {
        return Err(Error::Recurrence(format!(
            "override {}: {}",
            d.key, d.reason
        )));
    }
    Ok(found.iter().map(Candidate::end_utc).max())
}

fn extra_end(
    engine: &dyn RecurrenceEngine,
    e: &Event,
    floating: &Zone,
    base: DateTime<Utc>,
) -> Result<DateTime<Utc>> {
    // overrides of a non-recurring event can still add instances
    if e.recurrence_overrides.is_empty() {
        return Ok(base);
    }
    let window = (
        back(base, Duration::days(366 * 200)),
        forward(base, Duration::days(366 * 200)),
    );
    Ok(last_end(engine, e, window, floating)?.map_or(base, |end| end.max(base)))
}
