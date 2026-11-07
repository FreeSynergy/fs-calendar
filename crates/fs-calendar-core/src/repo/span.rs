//! The time span of an event for the index and the envelope.
//!
//! Pattern: Strategy (client) — an event's span comes from its earliest instance (an override may lie
//! before the master start) and series end (via the `RecurrenceEngine`).

use chrono::{DateTime, Datelike, Utc};

use super::handle::CalendarRepo;
use crate::error::Result;
use crate::model::Event;
use crate::recur::{earliest_start, series_end_with};
use crate::time::fmt_utc;

/// The index value of an instant. A local time near the edge of years 0 to 9999 can leave them in UTC,
/// and then its text would no longer sort: `-0001-...` falls out of every range. So it stays at the
/// edge. The event keeps its real time in the envelope; only the order in the index is at stake.
pub(super) fn index_time(t: DateTime<Utc>) -> String {
    match t.year() {
        ..0 => "0000-01-01T00:00:00Z".into(),
        0..10_000 => fmt_utc(t),
        _ => crate::time::FOREVER.into(),
    }
}

/// `(start, end, unbounded)`, all UTC.
pub(super) struct Span {
    pub(super) start: DateTime<Utc>,
    pub(super) end: Option<DateTime<Utc>>,
    pub(super) unbounded: bool,
}

impl CalendarRepo {
    /// Time span for the index: `(start, end, unbounded)`, all UTC. A series ends at its last
    /// instance, or is unbounded (no `count` / `until`).
    pub(super) fn time_index(&self, e: &Event) -> Result<Span> {
        let start = earliest_start(e, &self.floating)?;
        let end = series_end_with(&*self.engine, e, &self.floating)?;
        Ok(Span {
            start,
            end,
            unbounded: end.is_none(),
        })
    }
}
