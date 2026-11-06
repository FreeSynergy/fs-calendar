//! Expansion of an event into its instances, and the end of a series.
//!
//! Pattern: Template method + Iterator-style generation — the fixed skeleton (collect the rule's
//! starts, add override keys, drop exclusions, patch, convert with our own zone, filter by window)
//! is here; the rule itself is evaluated by the [`RecurrenceEngine`] strategy.

use std::collections::BTreeSet;

use chrono::{DateTime, Duration, NaiveDateTime, Utc};
use serde_json::Value;

use super::engine::RecurrenceEngine;
use super::occurrence::{Occurrence, duration_of, first_span, zone_of};
use super::patch::apply_patch;
use super::rrule_engine::RRuleEngine;
use crate::error::{Error, Result};
use crate::model::Event;
use crate::time::{Zone, fmt_local, parse_local};

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

/// All instances of `e` that overlap `[from, to)`, ordered by start, at most `limit`.
///
/// Zero-length instances count when `from <= start < to`. Instances excluded by
/// `recurrenceOverrides` are omitted; patched ones come back patched; override keys that the rule
/// does not produce are extra instances (RDATE). Floating events are interpreted in `floating`.
///
/// # Panics
/// Never in practice: instances are built as JSON objects before they are patched.
pub fn expand_with(
    engine: &dyn RecurrenceEngine,
    e: &Event,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
    floating: &Zone,
    limit: usize,
) -> Result<Vec<Occurrence>> {
    let zone = zone_of(e, floating)?;
    let d = duration_of(e)?;
    let margin = Duration::seconds(d.max_secs() + 2 * 86_400);
    let mut starts: BTreeSet<NaiveDateTime> = rule_starts(engine, e, &zone, from - margin, to)?
        .into_iter()
        .collect();
    for k in e.recurrence_overrides.keys() {
        starts.insert(parse_local(k)?); // extra instances; excluded ones are removed below
    }
    let master = Value::Object({
        let mut m = e.to_value().as_object().cloned().unwrap_or_default();
        for k in ["recurrenceRule", "recurrenceOverrides"] {
            m.remove(k);
        }
        m
    });
    let mut out = vec![];
    for local in starts {
        let key = fmt_local(local);
        let ov = e.recurrence_overrides.get(&key).and_then(Value::as_object);
        if ov.is_some_and(|o| o.get("excluded") == Some(&Value::Bool(true))) {
            continue;
        }
        let mut inst = master.clone();
        let io = inst.as_object_mut().unwrap();
        io.insert("recurrenceId".into(), Value::String(key.clone()));
        io.insert("start".into(), Value::String(key.clone()));
        if let Some(o) = ov {
            let mut patch = o.clone();
            patch.remove("excluded");
            inst = apply_patch(inst, &patch);
        }
        let ev = Event::from_value(inst)?;
        // a patched instance may move itself
        let (s_local, dur) = (
            ev.start
                .as_deref()
                .map(parse_local)
                .transpose()?
                .unwrap_or(local),
            duration_of(&ev)?,
        );
        let inst_zone = zone_of(&ev, floating)?;
        let (start_utc, end_utc) = (inst_zone.to_utc(s_local), dur.end_utc(&inst_zone, s_local));
        let overlaps = if end_utc > start_utc {
            start_utc < to && end_utc > from
        } else {
            start_utc >= from && start_utc < to
        };
        if overlaps {
            out.push(Occurrence {
                recurrence_id: key,
                start_utc,
                end_utc,
                event: ev,
                overridden: ov.is_some(),
            });
        }
    }
    out.sort_by_key(|o| (o.start_utc, o.recurrence_id.clone()));
    out.truncate(limit);
    Ok(out)
}

/// End of the last instance (UTC), or `None` if the series is unbounded (no `count` / `until`)
/// or the rule cannot be evaluated. Non-recurring events return their own end.
pub fn series_end_with(
    engine: &dyn RecurrenceEngine,
    e: &Event,
    floating: &Zone,
) -> Result<Option<DateTime<Utc>>> {
    let (first_start, first_end) = first_span(e, floating)?;
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
