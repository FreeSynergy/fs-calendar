//! An `Event` as typed values, and typed values applied to an `Event`.
//!
//! Pattern: Adapter (both ways) — the form speaks `Values` (texts by field name), the core speaks `Event`. `apply`
//! is the partial rule of the contract: a field missing from the input is unchanged, an empty list clears it, values
//! replace. Maps of JSCalendar objects (`locations`, `participants`, `links`) keep the objects whose address is still
//! in the list, so their other members (names, roles, answers) survive an edit.

use fs_admin_frontend::Values;
use fs_calendar_core::Event;

use super::objects::{address, objects, replace_objects, replace_participants};
use serde_json::Value;

/// A value that cannot be read: the field and the index of the value.
#[derive(Debug, PartialEq, Eq)]
pub struct Unreadable {
    /// Field name.
    pub field: String,
    /// Index of the value.
    pub index: usize,
}

fn put(out: &mut Values, name: &str, values: Vec<String>) {
    if !values.is_empty() {
        out.insert(name.to_owned(), values);
    }
}

/// The values of an event, only the fields that have one.
#[must_use]
pub fn to_values(e: &Event) -> Values {
    let mut out = Values::new();
    let one = |s: &Option<String>| s.iter().cloned().collect::<Vec<_>>();
    put(&mut out, "title", one(&e.title));
    put(&mut out, "description", one(&e.description));
    put(&mut out, "start", one(&e.start));
    put(&mut out, "duration", one(&e.duration));
    put(&mut out, "timeZone", one(&e.time_zone));
    put(&mut out, "endTimeZone", one(&e.end_time_zone));
    put(
        &mut out,
        "showWithoutTime",
        e.show_without_time
            .iter()
            .map(ToString::to_string)
            .collect(),
    );
    put(&mut out, "status", one(&e.status));
    put(&mut out, "freeBusyStatus", one(&e.free_busy_status));
    put(&mut out, "privacy", one(&e.privacy));
    put(
        &mut out,
        "priority",
        e.priority.iter().map(ToString::to_string).collect(),
    );
    let keywords: Vec<&str> = e
        .keywords
        .iter()
        .filter(|(_, on)| **on)
        .map(|(k, _)| k.as_str())
        .collect();
    put(
        &mut out,
        "keywords",
        if keywords.is_empty() {
            vec![]
        } else {
            vec![keywords.join(", ")]
        },
    );
    let places = objects(e, "locations");
    put(
        &mut out,
        "locations",
        places
            .values()
            .filter_map(|o| o.get("coordinates")?.as_str().map(str::to_owned))
            .collect(),
    );
    put(
        &mut out,
        "participants",
        e.participants.values().filter_map(address).collect(),
    );
    let links = objects(e, "links");
    put(
        &mut out,
        "links",
        links
            .values()
            .filter_map(|o| o.get("href")?.as_str().map(str::to_owned))
            .collect(),
    );
    put(
        &mut out,
        "color",
        e.extra
            .get("color")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .into_iter()
            .collect(),
    );
    out
}

/// The values sent for a field; `None` = not sent (unchanged), an empty slice = cleared.
fn sent<'a>(input: &'a Values, name: &str) -> Option<&'a [String]> {
    input.get(name).map(Vec::as_slice)
}

fn set_text(slot: &mut Option<String>, input: &Values, name: &str) {
    if let Some(v) = sent(input, name) {
        *slot = v.first().cloned();
    }
}

/// Apply `input` to `e` by the partial rule.
///
/// # Errors
/// The first value that is not a number or a yes/no (the choices are checked by the form).
pub fn apply(e: &mut Event, input: &Values) -> Result<(), Unreadable> {
    set_text(&mut e.title, input, "title");
    set_text(&mut e.description, input, "description");
    set_text(&mut e.start, input, "start");
    set_text(&mut e.duration, input, "duration");
    set_text(&mut e.time_zone, input, "timeZone");
    set_text(&mut e.end_time_zone, input, "endTimeZone");
    set_text(&mut e.status, input, "status");
    set_text(&mut e.free_busy_status, input, "freeBusyStatus");
    set_text(&mut e.privacy, input, "privacy");
    let bad = |field: &str| Unreadable {
        field: field.to_owned(),
        index: 0,
    };
    if let Some(v) = sent(input, "showWithoutTime") {
        e.show_without_time = v
            .first()
            .map(|t| t.parse().map_err(|_| bad("showWithoutTime")))
            .transpose()?;
    }
    if let Some(v) = sent(input, "priority") {
        e.priority = v
            .first()
            .map(|t| t.parse().map_err(|_| bad("priority")))
            .transpose()?;
    }
    if let Some(v) = sent(input, "keywords") {
        e.keywords = v
            .iter()
            .flat_map(|t| t.split(','))
            .map(str::trim)
            .filter(|k| !k.is_empty())
            .map(|k| (k.to_owned(), true))
            .collect();
    }
    if let Some(v) = input.get("locations") {
        replace_objects(e, "locations", "coordinates", "Location", v);
    }
    if let Some(v) = input.get("links") {
        replace_objects(e, "links", "href", "Link", v);
    }
    if let Some(v) = input.get("participants") {
        replace_participants(e, v);
    }
    if let Some(v) = sent(input, "color") {
        match v.first().cloned() {
            Some(c) => {
                e.extra.insert("color".into(), Value::String(c));
            }
            None => {
                e.extra.remove("color");
            }
        }
    }
    Ok(())
}
