//! The JSCalendar maps of an event that the description shows as lists: locations, links, participants.
//!
//! Pattern: Adapter helpers of `record.rs` — an edit of the list keeps the objects whose address is still in it, so
//! their other members (names, roles, answers) survive; objects this field cannot show (a place with a name only)
//! are kept too.

use std::collections::BTreeSet;

use fs_calendar_core::Event;
use fs_calendar_core::model::Participant;
use serde_json::{Map, Value, json};

pub(super) fn objects(event: &Event, key: &str) -> Map<String, Value> {
    event
        .extra
        .get(key)
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default()
}

pub(super) fn address(p: &Participant) -> Option<String> {
    p.calendar_address
        .clone()
        .or_else(|| p.email.as_ref().map(|e| format!("mailto:{e}")))
}

/// Replace the objects of `key` by one per value; an object whose `member` equals a value stays as it is, an object
/// without that member (a place with a name only) is kept too, since this field cannot show it.
pub(super) fn replace_objects(
    e: &mut Event,
    key: &str,
    member_name: &str,
    kind: &str,
    values: &[String],
) {
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

pub(super) fn replace_participants(e: &mut Event, values: &[String]) {
    let mut want: BTreeSet<&str> = values.iter().map(String::as_str).collect();
    e.participants
        .retain(|_, p| address(p).is_none_or(|a| want.remove(a.as_str())));
    for v in values {
        if !want.remove(v.as_str()) {
            continue;
        }
        let key = (1..=e.participants.len() + 1)
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
