//! Events whose time zone an import could not keep.
//!
//! Pattern: Specification over what calcard, the core's own iCalendar parser, reads in the file — until
//! the core says so itself, an event the import stored from a part whose `TZID` no zone answers to is
//! named, because it may now mean another time. A name calcard maps to a real zone, which the event
//! then has, is no loss. Only what this import stored is looked at, and what is marked is not called
//! by its title.

use calcard::icalendar::{
    ICalendar, ICalendarComponentType, ICalendarParameterName, ICalendarParameterValue,
    ICalendarProperty,
};
use fs_calendar_core::{Calendar, Zone};
use fs_store::ObjectId;
use tokio::runtime::Handle;

use crate::source::is_marked;
use crate::words::say;

/// The `TZID` of the start of the event itself, not of an override of it, and all `TZID`s of a part.
fn zone_names(part: &str) -> (Option<String>, Vec<String>) {
    let (mut master, mut all) = (None, Vec::new());
    let Ok(calendar) = ICalendar::parse(part) else {
        return (master, all);
    };
    let events = calendar
        .components
        .iter()
        .filter(|c| c.component_type == ICalendarComponentType::VEvent);
    for event in events {
        let is_override = event
            .entries
            .iter()
            .any(|e| e.name == ICalendarProperty::RecurrenceId);
        for entry in &event.entries {
            for param in &entry.params {
                if param.name == ICalendarParameterName::Tzid
                    && let ICalendarParameterValue::Text(name) = &param.value
                {
                    if !is_override && entry.name == ICalendarProperty::Dtstart {
                        master = Some(name.clone());
                    }
                    all.push(name.clone());
                }
            }
        }
    }
    (master, all)
}

/// Those of the events stored from `parts` (object id and the text it came from) that lost a zone, in
/// words.
pub(crate) fn lost_zones(
    calendar: &dyn Calendar,
    runtime: &Handle,
    parts: &[(ObjectId, String)],
) -> Vec<String> {
    parts
        .iter()
        .filter_map(|(id, text)| {
            let (master, names) = zone_names(text);
            let object = runtime.block_on(calendar.get(*id)).ok().flatten()?;
            // With a zone stored, the name on the start was understood, wherever else it appears.
            let understood = object
                .content
                .get("timeZone")
                .is_some()
                .then_some(master)
                .flatten();
            let lost = names
                .iter()
                .any(|name| Zone::parse(name).is_err() && understood.as_ref() != Some(name));
            if !lost {
                return None;
            }
            let uid = object.content["uid"].as_str().unwrap_or_default();
            let called = if is_marked(&object) {
                say::MARKED_LOST
            } else {
                &object.envelope.name
            };
            Some(format!("{called} ({uid})"))
        })
        .collect()
}
