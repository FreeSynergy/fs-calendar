//! The starts of an event that overlap a window, as light candidates.
//!
//! Pattern: Template method — collect the rule's starts, add the override keys, drop exclusions, keep
//! what overlaps the window. A plain start stays a time span; an `Event` is built for it only if it is
//! returned, so a series of 150 years of a large event never holds 55 000 copies. The rule itself is
//! evaluated by the [`RecurrenceEngine`] strategy.

use std::collections::BTreeSet;

use chrono::{DateTime, Duration, NaiveDateTime, Utc};

use super::engine::RecurrenceEngine;
use super::instance::{instance, is_excluded, master_value, override_of};
use super::occurrence::{Occurrence, duration_of, zone_of};
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

/// A start that overlaps the window: for most only its span, an `Event` is built for it only if it is
/// kept (a series of 150 years of a large event must not hold 55 000 copies), for an overridden one the
/// built instance, because its patch may move it.
pub(super) enum Candidate {
    Plain {
        local: NaiveDateTime,
        start_utc: DateTime<Utc>,
        end_utc: DateTime<Utc>,
    },
    Built(Box<Occurrence>),
}

impl Candidate {
    pub(super) fn order(&self) -> (DateTime<Utc>, String) {
        match self {
            Self::Plain {
                local, start_utc, ..
            } => (*start_utc, fmt_local(*local)),
            Self::Built(o) => (o.start_utc, o.recurrence_id.clone()),
        }
    }

    pub(super) fn end_utc(&self) -> DateTime<Utc> {
        match self {
            Self::Plain { end_utc, .. } => *end_utc,
            Self::Built(o) => o.end_utc,
        }
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

/// The starts overlapping `[from, to)` as light candidates, and the overrides that could not be built.
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
    let master = master_value(e);
    let mut found = Vec::new();
    for local in starts {
        let ov = override_of(e, &fmt_local(local));
        if is_excluded(ov) {
            continue;
        }
        if ov.is_some() {
            match instance(&master, ov, local, window, floating) {
                Ok(Some(built)) => found.push(Candidate::Built(Box::new(built))),
                Ok(None) => {}
                Err(err) => damaged.push(Damage {
                    key: fmt_local(local),
                    reason: err.to_string(),
                }),
            }
            continue;
        }
        let (start_utc, end_utc) = (zone.to_utc(local), length.end_utc(&zone, local));
        let overlaps = if end_utc > start_utc {
            start_utc < to && end_utc > from
        } else {
            start_utc >= from && start_utc < to
        };
        if overlaps {
            found.push(Candidate::Plain {
                local,
                start_utc,
                end_utc,
            });
        }
    }
    Ok((found, damaged))
}
