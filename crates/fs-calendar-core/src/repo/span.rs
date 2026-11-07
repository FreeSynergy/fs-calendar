//! The time span of an event for the index and the envelope.
//!
//! Pattern: Strategy (client) — an event's span comes from its earliest instance (an override may lie
//! before the master start) and series end (via the `RecurrenceEngine`).

use chrono::{DateTime, Utc};

use super::handle::CalendarRepo;
use crate::error::Result;
use crate::model::Event;
use crate::recur::{earliest_start, series_end_with};

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
