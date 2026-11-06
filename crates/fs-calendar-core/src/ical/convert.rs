//! iCalendar <-> JSCalendar conversion through `calcard`.
//!
//! Pattern: Adapter — `calcard` (MIT OR Apache-2.0) maps iCalendar to JSCalendar and back; we hand
//! it JSON text and own the typed result. Imported text is kept as the original (ADR 0030).

use calcard::icalendar::ICalendar;
use calcard::jscalendar::JSCalendar;
use serde_json::{Value, json};

use crate::error::{Error, Result};
use crate::model::CalendarObject;
use crate::upgrade::upgrade;

/// Media type of the stored original.
pub const ICS_MEDIA_TYPE: &str = "text/calendar";

/// Convert one iCalendar text to upgraded JSCalendar 2.0 objects (several if the text holds several
/// components) plus upgrade notes.
pub fn parse_ics(text: &str) -> Result<(Vec<CalendarObject>, Vec<String>)> {
    let ical =
        ICalendar::parse(text).map_err(|e| Error::ICalendar(format!("cannot parse: {e:?}")))?;
    let js = ical.into_jscalendar::<String, String>();
    let value: Value = serde_json::from_str(&js.to_string_pretty())?;
    let up = upgrade(value);
    let entries = match up.value.get("entries") {
        Some(Value::Array(a)) => a.clone(),
        _ => vec![up.value.clone()],
    };
    let mut out = vec![];
    for e in entries {
        if matches!(
            e.get("@type").and_then(Value::as_str),
            Some("Event" | "Task")
        ) {
            out.push(CalendarObject::from_value(e)?);
        }
    }
    Ok((out, up.notes))
}

/// Export objects as one iCalendar stream.
pub fn to_ics(objects: &[CalendarObject]) -> Result<String> {
    // calcard's JSCalendar model has no top-level `version` on entries; the rest is accepted as is.
    let entries: Vec<Value> = objects
        .iter()
        .map(|o| {
            let mut v = o.to_value();
            if let Some(m) = v.as_object_mut() {
                m.remove("version");
            }
            prepare_participants(&mut v);
            v
        })
        .collect();
    let group = json!({
        "@type": "Group",
        "uid": format!("urn:uuid:{}", fs_store::ObjectId::new()),
        "updated": crate::now_utc(),
        "prodId": "-//FreeSynergy//fs-calendar-core//EN",
        "entries": entries,
    });
    let text = group.to_string();
    // calcard asserts internally on malformed input; a panic must never take the program down.
    std::panic::catch_unwind(|| {
        let js = JSCalendar::<String, String>::parse(&text)
            .map_err(|e| Error::ICalendar(format!("JSCalendar rejected: {e}")))?;
        let ical = js
            .into_icalendar()
            .ok_or_else(|| Error::ICalendar("cannot be expressed as iCalendar".into()))?;
        Ok(ical.to_string())
    })
    .unwrap_or_else(|_| {
        Err(Error::ICalendar(
            "the converter failed on this input".into(),
        ))
    })
}

/// iCalendar attendees need an address: fill `calendarAddress` from `email` or `sendTo.imip`, and
/// leave out participants that have none (they cannot be expressed as ATTENDEE).
fn prepare_participants(v: &mut Value) {
    let Some(ps) = v.get_mut("participants").and_then(Value::as_object_mut) else {
        return;
    };
    ps.retain(|_, p| {
        let Some(o) = p.as_object_mut() else {
            return false;
        };
        if !o.contains_key("calendarAddress") {
            let addr = o
                .get("email")
                .and_then(Value::as_str)
                .map(|e| format!("mailto:{e}"))
                .or_else(|| {
                    o.get("sendTo")
                        .and_then(|s| s.get("imip"))
                        .and_then(Value::as_str)
                        .map(str::to_string)
                });
            if let Some(a) = addr {
                o.insert("calendarAddress".into(), Value::String(a));
            }
        }
        o.contains_key("calendarAddress")
    });
    if ps.is_empty() {
        v.as_object_mut().map(|m| m.remove("participants"));
    }
}
