//! The time zones an iCalendar text names, and whether the import understood them.
//!
//! Pattern: Specification — a pure predicate over what `calcard` reads — a `TZID` that neither a `VTIMEZONE`
//! of the text nor the zone database answers to is converted without a zone, which means another time
//! (floating, or the clock time of another zone). An event that names one is refused with the name
//! instead of being stored at the wrong time. A name `calcard` itself resolves (an alias, a zone the
//! text defines) is no loss: the converted event carries that zone.

use calcard::icalendar::{
    ICalendar, ICalendarComponentType, ICalendarParameterName, ICalendarParameterValue,
};

/// Every `TZID` named by a `VEVENT` of the text, and the first of them that `calcard` cannot resolve.
pub fn unknown_zone(text: &str) -> Option<String> {
    let calendar = ICalendar::parse(text).ok()?;
    let resolver = calendar.build_tz_resolver();
    let events = calendar
        .components
        .iter()
        .filter(|c| c.component_type == ICalendarComponentType::VEvent);
    for event in events {
        for entry in &event.entries {
            for param in &entry.params {
                if param.name == ICalendarParameterName::Tzid
                    && let ICalendarParameterValue::Text(name) = &param.value
                    && resolver.resolve(name).is_none()
                {
                    return Some(name.clone());
                }
            }
        }
    }
    None
}
