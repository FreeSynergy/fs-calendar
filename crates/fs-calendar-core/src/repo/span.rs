//! The time span of an object for the index and the envelope.
//!
//! Pattern: Strategy (by object kind) — an event's span comes from its first instance and series
//! end (via the `RecurrenceEngine`), a task's from `start` / `due` / `estimatedDuration`.

use chrono::{DateTime, Utc};

use super::handle::CalendarRepo;
use crate::error::Result;
use crate::model::CalendarObject;
use crate::recur::{first_span, series_end_with};
use crate::time::{Dur, Zone, parse_local};

/// `(start, end, unbounded, due)`, all UTC.
pub(super) struct Span {
    pub(super) start: Option<DateTime<Utc>>,
    pub(super) end: Option<DateTime<Utc>>,
    pub(super) unbounded: bool,
    pub(super) due: Option<DateTime<Utc>>,
}

impl CalendarRepo {
    /// Time span for the index: `(start, end, unbounded, due)`, all UTC. A series ends at its
    /// last instance, or is unbounded (no `count` / `until`).
    pub(super) fn time_index(&self, obj: &CalendarObject) -> Result<Span> {
        match obj {
            CalendarObject::Event(e) => {
                let (start, _) = first_span(e, &self.floating)?;
                let end = series_end_with(&*self.engine, e, &self.floating)?;
                Ok(Span {
                    start: Some(start),
                    end,
                    unbounded: end.is_none(),
                    due: None,
                })
            }
            CalendarObject::Task(t) => {
                let zone = match t.time_zone.as_deref() {
                    Some(z) => Zone::parse(z)?,
                    None => self.floating.clone(),
                };
                let local = |s: &Option<String>| -> Result<Option<DateTime<Utc>>> {
                    s.as_deref()
                        .map(|x| parse_local(x).map(|l| zone.to_utc(l)))
                        .transpose()
                };
                let (start, due) = (local(&t.start)?, local(&t.due)?);
                let est = t
                    .estimated_duration
                    .as_deref()
                    .map(Dur::parse)
                    .transpose()?;
                let end = due.or_else(|| {
                    start
                        .zip(est)
                        .map(|(s, d)| d.end_utc(&Zone::Utc, s.naive_utc()))
                });
                Ok(Span {
                    start: start.or(due),
                    end: end.or(start),
                    unbounded: false,
                    due,
                })
            }
        }
    }
}
