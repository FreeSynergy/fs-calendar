//! One concrete instance of an event, and the first-instance helpers.
//!
//! Pattern: Value object — an [`Occurrence`] is the master event with its override applied plus
//! the UTC span; `zone_of` and `first_span` resolve an event's own zone and first instance.

use chrono::{DateTime, Utc};

use crate::error::{Error, Result};
use crate::model::Event;
use crate::time::{Dur, Zone, parse_local};

/// One concrete instance of an event.
#[derive(Clone, Debug, PartialEq)]
pub struct Occurrence {
    /// `LocalDateTime` key of this instance in the master's zone (the `recurrenceOverrides` key /
    /// `recurrenceId`).
    pub recurrence_id: String,
    /// Start of the instance in UTC.
    pub start_utc: DateTime<Utc>,
    /// End of the instance in UTC.
    pub end_utc: DateTime<Utc>,
    /// Instance after applying its override patch (no `recurrenceRule` / `recurrenceOverrides`;
    /// `recurrenceId` set).
    pub event: Event,
    /// Whether an override patch was applied.
    pub overridden: bool,
}

/// Zone of an event: its `timeZone`, else `floating` (the caller's default).
pub fn zone_of(e: &Event, floating: &Zone) -> Result<Zone> {
    match e.time_zone.as_deref() {
        Some(z) => Ok(Zone::parse(z)?),
        None => Ok(floating.clone()),
    }
}

pub(super) fn duration_of(e: &Event) -> Result<Dur> {
    Ok(e.duration
        .as_deref()
        .map(Dur::parse)
        .transpose()?
        .unwrap_or_default())
}

/// First start / end of the master instance in UTC.
pub fn first_span(e: &Event, floating: &Zone) -> Result<(DateTime<Utc>, DateTime<Utc>)> {
    let zone = zone_of(e, floating)?;
    let start = parse_local(
        e.start
            .as_deref()
            .ok_or_else(|| Error::Time("event without start".into()))?,
    )?;
    let d = duration_of(e)?;
    Ok((zone.to_utc(start), d.end_utc(&zone, start)))
}
