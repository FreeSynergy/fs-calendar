//! Expansion of an event into its instances, and the end of a series.
//!
//! Pattern: Template method + Iterator-style generation — the fixed skeleton (collect the rule's
//! starts, add override keys, drop exclusions, build each instance, filter by window) is here; the
//! rule itself is evaluated by the [`RecurrenceEngine`] strategy. A damaged override costs its own
//! instance: [`expand_lenient`] names it, [`expand_with`] is the strict form that fails on the first.

use std::collections::BTreeSet;

use chrono::{DateTime, Duration, NaiveDateTime, Utc};

use super::early::earliest_start;
use super::engine::RecurrenceEngine;
use super::instance::{instance, is_excluded, master_value, override_of};
use super::occurrence::{Occurrence, duration_of, zone_of};
use super::rrule_engine::RRuleEngine;
use super::steps::check_steps;
use crate::error::{Error, Result};
use crate::model::Event;
use crate::time::{Zone, fmt_local, parse_local};

/// An instance that could not be built, by its override key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Damage {
    /// The key of the override (or the text that should have been one).
    pub key: String,
    /// Why it failed.
    pub reason: String,
}

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

/// Local start times produced by the rule itself (always includes the master start) within
/// `[after, before]` (both UTC bounds, generous: callers filter exactly).
fn rule_starts(
    engine: &dyn RecurrenceEngine,
    e: &Event,
    zone: &Zone,
    after: DateTime<Utc>,
    before: DateTime<Utc>,
) -> Result<Vec<NaiveDateTime>> {
    let start = parse_local(
        e.start
            .as_deref()
            .ok_or_else(|| Error::Time("event without start".into()))?,
    )?;
    let mut out = BTreeSet::new();
    out.insert(start);
    if let Some(rule) = &e.recurrence_rule {
        check_steps(rule, start, zone.to_local(before))?;
        out.extend(engine.starts(rule, start, zone, after, before)?);
    }
    Ok(out.into_iter().collect())
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
/// An error is the event as a whole: its rule or its zone cannot be read.
pub fn expand_lenient(
    engine: &dyn RecurrenceEngine,
    e: &Event,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
    floating: &Zone,
    limit: usize,
) -> Result<Expansion> {
    let zone = zone_of(e, floating)?;
    let margin = Duration::seconds(duration_of(e)?.max_secs() + 2 * 86_400);
    let mut starts: BTreeSet<NaiveDateTime> = rule_starts(engine, e, &zone, from - margin, to)?
        .into_iter()
        .collect();
    let mut out = Expansion::default();
    for key in e.recurrence_overrides.keys() {
        match parse_local(key) {
            Ok(local) => {
                starts.insert(local); // extra instances; excluded ones are skipped below
            }
            Err(err) => out.damaged.push(Damage {
                key: key.clone(),
                reason: err.to_string(),
            }),
        }
    }
    let master = master_value(e);
    for local in starts {
        let ov = override_of(e, &fmt_local(local));
        if is_excluded(ov) {
            continue;
        }
        match instance(&master, ov, local, (from, to), floating) {
            Ok(Some(found)) => out.items.push(found),
            Ok(None) => {}
            Err(err) => out.damaged.push(Damage {
                key: fmt_local(local),
                reason: err.to_string(),
            }),
        }
    }
    out.items
        .sort_by_key(|o| (o.start_utc, o.recurrence_id.clone()));
    out.cut = out.items.len() > limit;
    out.items.truncate(limit);
    Ok(out)
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
    let far = first_start + Duration::days(366 * 400);
    let all = expand_with(engine, e, first_start, far, floating, usize::MAX)?;
    Ok(all.iter().map(|o| o.end_utc).max().or(Some(first_end)))
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
    let all = expand_with(
        engine,
        e,
        base - Duration::days(366 * 200),
        base + Duration::days(366 * 200),
        floating,
        usize::MAX,
    )?;
    Ok(all
        .iter()
        .map(|o| o.end_utc)
        .chain([base])
        .max()
        .unwrap_or(base))
}
