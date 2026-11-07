//! The starts of an event that overlap a window, as light candidates.
//!
//! Pattern: Template method — collect the rule's starts, add the override keys, drop exclusions, keep
//! what overlaps the window. Every start, overridden or not, stays a time span until it is returned. The rule itself is
//! evaluated by the [`RecurrenceEngine`] strategy.

use std::collections::BTreeSet;

use chrono::{DateTime, Duration, NaiveDateTime, Utc};
use serde_json::{Map, Value};

use super::engine::RecurrenceEngine;
use super::instance::{is_excluded, override_of};
use super::occurrence::{duration_of, zone_of};
use super::steps::check_steps;
use crate::error::{Error, Result};
use crate::model::Event;
use crate::time::{Dur, Zone, fmt_local, parse_local};

/// An instance that could not be built, by its override key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Damage {
    /// The key of the override (or the text that should have been one).
    pub key: String,
    /// Why it failed.
    pub reason: String,
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

/// A start that overlaps the window, as a time span. An `Event` is built for it only if it is returned
/// (an override's patch is read for the three members that move a span, never applied to a copy of the
/// master), so a series of 150 years, or 2000 overrides, of a large event never holds that many copies.
pub(super) struct Candidate {
    pub(super) local: NaiveDateTime,
    pub(super) start_utc: DateTime<Utc>,
    pub(super) end_utc: DateTime<Utc>,
}

impl Candidate {
    pub(super) fn order(&self) -> (DateTime<Utc>, String) {
        (self.start_utc, fmt_local(self.local))
    }
}

/// `t` moved back by `by`, at the smallest instant if that is further than the type holds.
pub(super) fn back(t: DateTime<Utc>, by: Duration) -> DateTime<Utc> {
    t.checked_sub_signed(by).unwrap_or(DateTime::<Utc>::MIN_UTC)
}

/// `t` moved forward by `by`, at the largest instant if that is further than the type holds.
pub(super) fn forward(t: DateTime<Utc>, by: Duration) -> DateTime<Utc> {
    t.checked_add_signed(by).unwrap_or(DateTime::<Utc>::MAX_UTC)
}

/// A member of a patch: not there, removed (`null`), or text.
enum Member<'a> {
    Absent,
    Removed,
    Text(&'a str),
}

fn member<'a>(patch: &'a Map<String, Value>, name: &str) -> Result<Member<'a>> {
    match patch.get(name) {
        None => Ok(Member::Absent),
        Some(Value::Null) => Ok(Member::Removed),
        Some(Value::String(text)) => Ok(Member::Text(text.as_str())),
        Some(_) => Err(Error::Time(format!("{name} of an override is not text"))),
    }
}

/// The span of an overridden start: `start`, `timeZone` and `duration` of its patch, else the master's.
fn override_span(
    patch: &Map<String, Value>,
    key: NaiveDateTime,
    master: (&Zone, Dur),
    floating: &Zone,
) -> Result<(DateTime<Utc>, DateTime<Utc>)> {
    let start = match member(patch, "start")? {
        Member::Text(text) => parse_local(text)?,
        _ => key,
    };
    let zone = match member(patch, "timeZone")? {
        Member::Text(name) => Zone::parse(name)?,
        Member::Removed => floating.clone(),
        Member::Absent => master.0.clone(),
    };
    let length = match member(patch, "duration")? {
        Member::Text(text) => Dur::parse(text)?,
        Member::Removed => Dur::default(),
        Member::Absent => master.1,
    };
    Ok((zone.to_utc(start), length.end_utc(&zone, start)))
}

/// Whether a span overlaps the window; a span of length zero counts when `from <= start < to`.
fn overlaps(span: (DateTime<Utc>, DateTime<Utc>), window: (DateTime<Utc>, DateTime<Utc>)) -> bool {
    let ((start, end), (from, to)) = (span, window);
    if end > start {
        start < to && end > from
    } else {
        start >= from && start < to
    }
}

/// The starts overlapping `[from, to)` as light candidates, and the overrides that could not be read.
pub(super) fn candidates(
    engine: &dyn RecurrenceEngine,
    e: &Event,
    window: (DateTime<Utc>, DateTime<Utc>),
    floating: &Zone,
) -> Result<(Vec<Candidate>, Vec<Damage>)> {
    let (from, to) = window;
    let zone = zone_of(e, floating)?;
    let length = duration_of(e)?;
    let margin = Duration::seconds(length.max_secs() + 2 * 86_400);
    let mut starts: BTreeSet<NaiveDateTime> =
        rule_starts(engine, e, &zone, back(from, margin), to)?
            .into_iter()
            .collect();
    let mut damaged = Vec::new();
    for key in e.recurrence_overrides.keys() {
        match parse_local(key) {
            Ok(local) => {
                starts.insert(local); // extra instances; excluded ones are skipped below
            }
            Err(err) => damaged.push(Damage {
                key: key.clone(),
                reason: err.to_string(),
            }),
        }
    }
    let mut found = Vec::new();
    for local in starts {
        let key = fmt_local(local);
        let ov = override_of(e, &key);
        if is_excluded(ov) {
            continue;
        }
        let span = match ov {
            Some(patch) => override_span(patch, local, (&zone, length), floating),
            None => Ok((zone.to_utc(local), length.end_utc(&zone, local))),
        };
        match span {
            Ok(span) if overlaps(span, window) => found.push(Candidate {
                local,
                start_utc: span.0,
                end_utc: span.1,
            }),
            Ok(_) => {}
            Err(err) => damaged.push(Damage {
                key,
                reason: err.to_string(),
            }),
        }
    }
    Ok((found, damaged))
}
