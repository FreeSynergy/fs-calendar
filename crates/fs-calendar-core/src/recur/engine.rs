//! The [`RecurrenceEngine`] contract.
//!
//! Pattern: Strategy — the engine only answers "which local start times does this rule produce";
//! overrides, zone conversion, durations and window filtering stay in `expand`, so `rrule` can be
//! replaced (another crate, a hand-written engine, a fake in tests) without touching them.

use chrono::{DateTime, NaiveDateTime, Utc};

use crate::error::Result;
use crate::model::RecurrenceRule;
use crate::time::Zone;

/// Evaluates a JSCalendar recurrence rule.
pub trait RecurrenceEngine: Send + Sync {
    /// Local (wall-clock) start times produced by `rule` for an event starting at `start` in
    /// `zone`, between `after` and `before` (UTC bounds, generous: the caller filters exactly).
    /// The master `start` itself may or may not be included; the caller always adds it.
    fn starts(
        &self,
        rule: &RecurrenceRule,
        start: NaiveDateTime,
        zone: &Zone,
        after: DateTime<Utc>,
        before: DateTime<Utc>,
    ) -> Result<Vec<NaiveDateTime>>;
}
