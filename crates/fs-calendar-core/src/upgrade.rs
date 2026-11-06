//! Upgrade of RFC 8984 (JSCalendar 1.0) data to JSCalendar 2.0.
//!
//! Pattern: Pipeline (chain of small rewrite steps) — each step moves one family of obsolete
//! properties and reports a note; the run is idempotent, so 2.0 data passes through unchanged.
//!
//! * `recurrenceRules` (array) -> `recurrenceRule` = first element; further rules go to the vendor
//!   property `freesynergy.net:legacy` and the report says so.
//! * Properties that 2.0 made obsolete are moved into `freesynergy.net:legacy` instead of being dropped:
//!   `timeZones`, `excludedRecurrenceRules`, `Link.cid`, `Location.timeZone` / `relativeTo`,
//!   `Participant.language` / `locationId`, fractional seconds, custom time zone ids.
//! * A trailing `Z` on `start` / `due` becomes `timeZone: "Etc/UTC"` (2.0 local date-times have no zone suffix).
//! * `version: "2.0"` is set. Groups are upgraded entry by entry. Already-2.0 data passes through unchanged
//!   (idempotent).
//!
//! Reserved-in-2.0 properties (`sendTo`, `replyTo`, `localizations`, iTIP scheduling properties) are
//! **kept in place** (see `DESIGN.md`).

use serde_json::{Map, Value, json};

use crate::model::{LEGACY_KEY, VERSION};

/// Result of an upgrade.
#[derive(Clone, Debug, PartialEq)]
pub struct Upgraded {
    /// The upgraded JSCalendar value (a `Group` is upgraded entry by entry).
    pub value: Value,
    /// Human-readable notes for everything that was moved, merged or dropped.
    pub notes: Vec<String>,
}

/// Upgrade RFC 8984 data to JSCalendar 2.0; already-2.0 data passes through unchanged.
pub fn upgrade(value: Value) -> Upgraded {
    let mut notes = vec![];
    let value = up(value, &mut notes, "");
    Upgraded { value, notes }
}

fn up(mut v: Value, notes: &mut Vec<String>, at: &str) -> Value {
    let Some(o) = v.as_object_mut() else { return v };
    if o.get("@type").and_then(Value::as_str) == Some("Group") {
        if let Some(Value::Array(entries)) = o.get_mut("entries") {
            let taken = std::mem::take(entries);
            *entries = taken
                .into_iter()
                .enumerate()
                .map(|(i, e)| up(e, notes, &format!("{at}/entries/{i}")))
                .collect();
        }
        return v;
    }
    let mut legacy = match o.remove(LEGACY_KEY) {
        Some(Value::Object(m)) => m,
        _ => Map::new(),
    };
    let was_2 = o.get("version").and_then(Value::as_str) == Some(VERSION);

    if let Some(Value::Array(mut rules)) = o.remove("recurrenceRules") {
        if !rules.is_empty() {
            let first = rules.remove(0);
            if !o.contains_key("recurrenceRule") {
                o.insert("recurrenceRule".into(), first);
            }
            if !rules.is_empty() {
                notes.push(format!("{at}: {} further recurrence rule(s) moved to {LEGACY_KEY}; 2.0 allows only one", rules.len()));
                legacy.insert("recurrenceRules".into(), Value::Array(rules));
            }
        }
    }
    for key in ["excludedRecurrenceRules", "timeZones"] {
        if let Some(x) = o.remove(key) {
            notes.push(format!(
                "{at}: `{key}` is obsolete in 2.0, moved to {LEGACY_KEY}"
            ));
            legacy.insert(key.into(), x);
        }
    }
    // custom (non-IANA) time zone ids cannot be expressed
    for key in ["timeZone", "recurrenceIdTimeZone", "endTimeZone"] {
        if o.get(key)
            .and_then(Value::as_str)
            .is_some_and(|z| z.starts_with('/'))
        {
            let x = o.remove(key).unwrap();
            notes.push(format!("{at}: custom time zone in `{key}` cannot be used in 2.0, moved to {LEGACY_KEY} (the object is now floating)"));
            legacy.insert(key.into(), x);
        }
    }
    // zone suffix on local date-times
    for key in ["start", "due"] {
        if let Some(s) = o.get(key).and_then(Value::as_str).map(str::to_string) {
            if let Some(local) = s.strip_suffix('Z') {
                if !o.contains_key("timeZone") {
                    o.insert("timeZone".into(), json!("Etc/UTC"));
                }
                o.insert(key.into(), json!(strip_fraction(local)));
                notes.push(format!(
                    "{at}: `{key}` had a Z suffix; now local time in Etc/UTC"
                ));
            } else if s.contains('.') {
                legacy.insert(format!("{key}Original"), json!(s));
                o.insert(key.into(), json!(strip_fraction(&s)));
                notes.push(format!("{at}: fractional seconds removed from `{key}`"));
            }
        }
    }
    if let Some(Value::Object(ov)) = o.get_mut("recurrenceOverrides") {
        let old = std::mem::take(ov);
        for (k, val) in old {
            let k2 = strip_fraction(k.strip_suffix('Z').unwrap_or(&k));
            ov.insert(k2, val);
        }
    }
    // obsolete members of sub-objects
    move_sub(
        o,
        "locations",
        &["timeZone", "relativeTo"],
        &mut legacy,
        notes,
        at,
    );
    move_sub(
        o,
        "participants",
        &["language", "locationId"],
        &mut legacy,
        notes,
        at,
    );
    move_sub(o, "links", &["cid"], &mut legacy, notes, at);

    if !was_2 {
        notes.push(format!("{at}: version set to 2.0"));
    }
    o.insert("version".into(), json!(VERSION));
    if !legacy.is_empty() {
        o.insert(LEGACY_KEY.into(), Value::Object(legacy));
    }
    v
}

fn strip_fraction(s: &str) -> String {
    match s.split_once('.') {
        Some((a, _)) => a.to_string(),
        None => s.to_string(),
    }
}

fn move_sub(
    o: &mut Map<String, Value>,
    field: &str,
    props: &[&str],
    legacy: &mut Map<String, Value>,
    notes: &mut Vec<String>,
    at: &str,
) {
    let Some(Value::Object(items)) = o.get_mut(field) else {
        return;
    };
    for (id, item) in items.iter_mut() {
        let Some(io) = item.as_object_mut() else {
            continue;
        };
        for p in props {
            if let Some(x) = io.remove(*p) {
                notes.push(format!(
                    "{at}: `{field}/{id}/{p}` is obsolete in 2.0, moved to {LEGACY_KEY}"
                ));
                legacy.insert(format!("{field}/{id}/{p}"), x);
            }
        }
    }
}
