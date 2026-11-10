//! An `Event` as typed values, and typed values applied to an `Event`.
//!
//! Pattern: Adapter (both ways) — the form speaks `Values` (texts by field name), the core speaks `Event`. `apply`
//! is the partial rule of the contract: a field missing from the input is unchanged, an empty list clears it, values
//! replace. Maps of JSCalendar objects (`locations`, `participants`, `links`) keep the objects whose address is still
//! in the list, so their other members (names, roles, answers) survive an edit.

use std::collections::BTreeSet;

use fs_admin_frontend::Values;
use fs_calendar_core::Event;
use fs_calendar_core::model::Participant;
use serde_json::{Map, Value, json};

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

fn objects(event: &Event, key: &str) -> Map<String, Value> {
    event
        .extra
        .get(key)
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default()
}

fn address(p: &Participant) -> Option<String> {
    p.calendar_address
        .clone()
        .or_else(|| p.email.as_ref().map(|e| format!("mailto:{e}")))
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

fn first(input: &Values, name: &str) -> Option<Option<String>> {
    input.get(name).map(|v| v.first().cloned())
}

fn set_text(slot: &mut Option<String>, input: &Values, name: &str) {
    if let Some(v) = first(input, name) {
        *slot = v;
    }
}

/// Replace the objects of `key` by one per value; an object whose `member` equals a value stays as it is, an object
/// without that member (a place with a name only) is kept too, since this field cannot show it.
fn replace_objects(e: &mut Event, key: &str, member_name: &str, kind: &str, values: &[String]) {
    let old = objects(e, key);
    let mut out = Map::new();
    let mut want: BTreeSet<&str> = values.iter().map(String::as_str).collect();
    for (k, o) in old {
        match o.get(member_name).and_then(Value::as_str) {
            None => {
                out.insert(k, o);
            }
            Some(m) if want.remove(m) => {
                out.insert(k, o);
            }
            Some(_) => {}
        }
    }
    for v in values {
        if !want.remove(v.as_str()) {
            continue;
        }
        let k = format!("k{}", out.len() + 1);
        let mut k = k;
        while out.contains_key(&k) {
            k.push('x');
        }
        out.insert(k, json!({ "@type": kind, member_name: v }));
    }
    if out.is_empty() {
        e.extra.remove(key);
    } else {
        e.extra.insert(key.to_owned(), Value::Object(out));
    }
}

fn replace_participants(e: &mut Event, values: &[String]) {
    let mut want: BTreeSet<&str> = values.iter().map(String::as_str).collect();
    e.participants
        .retain(|_, p| address(p).is_some_and(|a| want.remove(a.as_str())) || address(p).is_none());
    for v in values {
        if !want.remove(v.as_str()) {
            continue;
        }
        let key = (1..)
            .map(|i| format!("p{i}"))
            .find(|k| !e.participants.contains_key(k))
            .unwrap_or_default();
        let mut p = Participant {
            type_: Some("Participant".into()),
            kind: Some("individual".into()),
            calendar_address: Some(v.clone()),
            participation_status: Some("needs-action".into()),
            ..Participant::default()
        };
        p.email = v.strip_prefix("mailto:").map(str::to_owned);
        p.roles.insert("attendee".into(), true);
        e.participants.insert(key, p);
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
    if let Some(v) = first(input, "showWithoutTime") {
        e.show_without_time = v
            .map(|t| t.parse().map_err(|_| bad("showWithoutTime")))
            .transpose()?;
    }
    if let Some(v) = first(input, "priority") {
        e.priority = v
            .map(|t| t.parse().map_err(|_| bad("priority")))
            .transpose()?;
    }
    if let Some(v) = first(input, "keywords") {
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
    if let Some(v) = first(input, "color") {
        match v {
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
