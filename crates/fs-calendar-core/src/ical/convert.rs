//! iCalendar `VEVENT`s as typed events.
//!
//! Pattern: Adapter — `fs-jscalendar` does the conversion on JSON values; this file picks our
//! component kind and our type. `VTODO`s of the same text belong to `fs-tasks`.

use fs_jscalendar::ical::{self, Component};
pub use fs_jscalendar::ical::{ICS_MEDIA_TYPE, IcsPart};

use crate::error::Result;
use crate::model::Event;

/// Split an `.ics` text into per-UID parts of `VEVENT` components.
pub fn split_ics(text: &str) -> Vec<IcsPart> {
    ical::split_ics(text, Component::Event)
}

/// Convert one iCalendar text to upgraded JSCalendar 2.0 events (the `VEVENT` components; `VTODO`s
/// belong to fs-tasks and are ignored) plus upgrade notes.
pub fn parse_ics(text: &str) -> Result<(Vec<Event>, Vec<String>)> {
    let (entries, notes) = ical::parse_entries(text, Event::TYPE)?;
    let events = entries
        .into_iter()
        .map(Event::from_value)
        .collect::<fs_jscalendar::Result<_>>()?;
    Ok((events, notes))
}

/// Export events as one iCalendar stream.
pub fn to_ics(events: &[Event]) -> Result<String> {
    Ok(ical::write_ics(
        events.iter().map(Event::to_value),
        "-//FreeSynergy//fs-calendar-core//EN",
    )?)
}
