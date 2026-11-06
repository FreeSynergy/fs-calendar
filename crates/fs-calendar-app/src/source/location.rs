//! The location of an event: `locations`, a map the core keeps as it is.
//!
//! Pattern: Adapter over untyped JSON — the form shows the name of the first location. A change touches
//! that name only; an emptied name takes the entry with it only if nothing else is on the entry, so
//! coordinates, links and vendor properties stay.

use fs_calendar_core::Event;
use serde_json::{Map, Value, json};

const LOCATIONS: &str = "locations";

fn entries(event: &Event) -> Option<&Map<String, Value>> {
    event.extra.get(LOCATIONS)?.as_object()
}

/// The name of the first location by key, and how many there are.
pub(super) fn first_name(event: &Event) -> (String, usize) {
    let Some(map) = entries(event) else {
        return (String::new(), 0);
    };
    let first = map.keys().min();
    let name = first
        .and_then(|key| map[key]["name"].as_str())
        .unwrap_or_default();
    (name.to_string(), map.len())
}

/// Put `name` on the first location, making one if there is none; an empty `name` takes it away.
pub(super) fn set_first_name(event: &mut Event, name: &str) {
    if name.is_empty() {
        clear_first(event);
        return;
    }
    let entry = event
        .extra
        .entry(LOCATIONS.to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    let Some(map) = entry.as_object_mut() else {
        return;
    };
    let first = map.keys().min().cloned();
    match first
        .and_then(|key| map.get_mut(&key))
        .and_then(Value::as_object_mut)
    {
        Some(location) => {
            location.insert("name".to_string(), json!(name));
        }
        None => {
            map.insert("l1".to_string(), json!({"@type": "Location", "name": name}));
        }
    }
}

fn clear_first(event: &mut Event) {
    let Some(map) = event
        .extra
        .get_mut(LOCATIONS)
        .and_then(Value::as_object_mut)
    else {
        return;
    };
    let Some(first) = map.keys().min().cloned() else {
        return;
    };
    let nothing_else = map[&first]
        .as_object()
        .is_some_and(|location| location.keys().all(|key| key == "name" || key == "@type"));
    if nothing_else {
        map.remove(&first);
    } else if let Some(location) = map.get_mut(&first).and_then(Value::as_object_mut) {
        location.remove("name");
    }
    if map.is_empty() {
        event.extra.remove(LOCATIONS);
    }
}
