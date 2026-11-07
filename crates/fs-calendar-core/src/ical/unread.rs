//! What an import could not read, told to a person.
//!
//! Pattern: Specification — one rule per property of a `VEVENT`. The conversion drops a value it cannot
//! read, so the event only lacks the member: a series becomes a single event, a garbled end gives an
//! event of length zero. Only the component's own lines count, unfolded: an alarm has a `DURATION` and
//! a time zone a `DTSTART` of their own.

use chrono::NaiveDateTime;
use fs_jscalendar::ical::unfolded_lines;

use crate::model::Event;
use crate::time::Dur;

/// The own lines of each `VEVENT` in `text`: without its nested components.
fn own_lines(text: &str) -> Vec<Vec<String>> {
    let (mut events, mut own, mut depth) = (vec![], vec![], 0usize);
    for line in unfolded_lines(text) {
        let upper = line.to_ascii_uppercase();
        if depth == 0 {
            if upper == "BEGIN:VEVENT" {
                depth = 1;
            }
        } else if upper.starts_with("BEGIN:") {
            depth += 1;
        } else if upper.starts_with("END:") {
            depth -= 1;
            if depth == 0 {
                events.push(std::mem::take(&mut own));
            }
        } else if depth == 1 {
            own.push(line);
        }
    }
    events
}

/// The value of property `name`, if the lines have it.
fn value<'a>(own: &'a [String], name: &str) -> Option<&'a str> {
    own.iter().find_map(|line| {
        let head = line.get(..name.len())?;
        let rest = line[name.len()..].chars().next()?;
        if !head.eq_ignore_ascii_case(name) || !matches!(rest, ':' | ';') {
            return None;
        }
        Some(line.split_once(':').map_or("", |(_, v)| v))
    })
}

/// Whether `value` is a date or a date-time of iCalendar, floating or UTC.
fn is_time(value: &str) -> bool {
    NaiveDateTime::parse_from_str(value, "%Y%m%dT%H%M%SZ").is_ok()
        || NaiveDateTime::parse_from_str(value, "%Y%m%dT%H%M%S").is_ok()
        || chrono::NaiveDate::parse_from_str(value, "%Y%m%d").is_ok()
}

/// A note for each property of `text` the conversion left out of its event.
pub(crate) fn unread_properties(text: &str, events: &[Event]) -> Vec<String> {
    let mut notes = vec![];
    for own in own_lines(text) {
        // An instance of a series is no event of its own; what it says stays in the original.
        if value(&own, "RECURRENCE-ID").is_some() {
            continue;
        }
        let uid = value(&own, "UID");
        let Some(e) = events.iter().find(|e| Some(e.uid.as_str()) == uid) else {
            continue;
        };
        let duration = e.duration.as_deref().is_some_and(|d| Dur::parse(d).is_ok());
        for (name, read) in [
            ("DTSTART", e.start.is_some()),
            ("DTEND", value(&own, "DTEND").is_none_or(is_time)),
            ("DURATION", duration),
            ("RRULE", e.recurrence_rule.is_some()),
        ] {
            if !read && value(&own, name).is_some() {
                notes.push(format!("{}: {name} could not be read", e.uid));
            }
        }
    }
    notes
}
