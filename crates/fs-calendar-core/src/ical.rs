//! iCalendar (RFC 5545) import/export through `calcard` 0.3.14 (MIT OR Apache-2.0), which maps
//! iCalendar <-> JSCalendar. Imported text is kept as the original (ADR 0030).

use std::collections::BTreeMap;

use calcard::icalendar::ICalendar;
use calcard::jscalendar::JSCalendar;
use serde_json::{Value, json};

use crate::error::{Error, Result};
use crate::model::CalendarObject;
use crate::upgrade::upgrade;

pub const ICS_MEDIA_TYPE: &str = "text/calendar";

/// The part of an `.ics` stream that belongs to one `UID` (master + its overrides, with the
/// calendar header and all `VTIMEZONE`s).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IcsPart {
    pub uid: String,
    /// Exact source text: the whole input if it holds one UID, otherwise header + `VTIMEZONE`s +
    /// the UID's components + `END:VCALENDAR`, every line copied unchanged.
    pub text: String,
}

fn block_name(line: &str) -> Option<String> {
    let t = line.trim();
    (t.len() > 6 && t[..6].eq_ignore_ascii_case("BEGIN:")).then(|| t[6..].to_ascii_uppercase())
}

fn uid_of(block: &[&str]) -> Option<String> {
    let mut found: Option<String> = None;
    for (i, l) in block.iter().enumerate() {
        let up = l.get(..4).map(str::to_ascii_uppercase);
        if matches!(up.as_deref(), Some("UID:") | Some("UID;")) {
            let mut v = l[l.find(':')? + 1..].trim_end_matches(['\r', '\n']).to_string();
            for cont in &block[i + 1..] {
                if cont.starts_with(' ') || cont.starts_with('\t') {
                    v.push_str(cont[1..].trim_end_matches(['\r', '\n']));
                } else {
                    break;
                }
            }
            found = Some(v);
            break;
        }
    }
    found
}

/// Split an `.ics` text into per-UID parts.
pub fn split_ics(text: &str) -> Vec<IcsPart> {
    let eol = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let (mut head, mut tz): (Vec<&str>, Vec<&str>) = (vec![], vec![]);
    let mut comps: Vec<(String, Vec<&str>)> = vec![]; // (uid, lines) in order
    let mut i = 0;
    let mut seen_component = false;
    while i < lines.len() {
        let l = lines[i];
        match block_name(l).as_deref() {
            Some("VCALENDAR") | None => {
                let t = l.trim();
                if !t.eq_ignore_ascii_case("END:VCALENDAR") && !seen_component {
                    head.push(l);
                }
                i += 1;
            }
            Some(name) => {
                let end = (i + 1..lines.len()).find(|&j| lines[j].trim().eq_ignore_ascii_case(&format!("END:{name}"))).unwrap_or(lines.len() - 1);
                let block = &lines[i..=end];
                match name {
                    "VTIMEZONE" => tz.extend_from_slice(block),
                    "VEVENT" | "VTODO" => {
                        seen_component = true;
                        if let Some(uid) = uid_of(block) {
                            comps.push((uid, block.to_vec()));
                        }
                    }
                    _ => {}
                }
                seen_component |= name != "VTIMEZONE";
                i = end + 1;
            }
        }
    }
    let mut by_uid: BTreeMap<String, Vec<&str>> = BTreeMap::new();
    let mut order = vec![];
    for (uid, b) in comps {
        if !by_uid.contains_key(&uid) {
            order.push(uid.clone());
        }
        by_uid.entry(uid).or_default().extend(b);
    }
    if order.len() == 1 {
        return vec![IcsPart { uid: order.remove(0), text: text.to_string() }];
    }
    order
        .into_iter()
        .map(|uid| {
            let mut t = String::new();
            head.iter().chain(tz.iter()).chain(by_uid[&uid].iter()).for_each(|l| t.push_str(l));
            if !t.ends_with('\n') {
                t.push_str(eol);
            }
            t.push_str("END:VCALENDAR");
            t.push_str(eol);
            IcsPart { uid, text: t }
        })
        .collect()
}

/// Convert one iCalendar text to upgraded JSCalendar 2.0 objects (several if the text holds several
/// components) plus upgrade notes.
pub fn parse_ics(text: &str) -> Result<(Vec<CalendarObject>, Vec<String>)> {
    let ical = ICalendar::parse(text).map_err(|e| Error::ICalendar(format!("cannot parse: {e:?}")))?;
    let js = ical.into_jscalendar::<String, String>();
    let value: Value = serde_json::from_str(&js.to_string_pretty())?;
    let up = upgrade(value);
    let entries = match up.value.get("entries") {
        Some(Value::Array(a)) => a.clone(),
        _ => vec![up.value.clone()],
    };
    let mut out = vec![];
    for e in entries {
        if matches!(e.get("@type").and_then(Value::as_str), Some("Event" | "Task")) {
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
        let js = JSCalendar::<String, String>::parse(&text).map_err(|e| Error::ICalendar(format!("JSCalendar rejected: {e}")))?;
        let ical = js.into_icalendar().ok_or_else(|| Error::ICalendar("cannot be expressed as iCalendar".into()))?;
        Ok(ical.to_string())
    })
    .unwrap_or_else(|_| Err(Error::ICalendar("the converter failed on this input".into())))
}

/// iCalendar attendees need an address: fill `calendarAddress` from `email` or `sendTo.imip`, and
/// leave out participants that have none (they cannot be expressed as ATTENDEE).
fn prepare_participants(v: &mut Value) {
    let Some(ps) = v.get_mut("participants").and_then(Value::as_object_mut) else { return };
    ps.retain(|_, p| {
        let Some(o) = p.as_object_mut() else { return false };
        if !o.contains_key("calendarAddress") {
            let addr = o
                .get("email")
                .and_then(Value::as_str)
                .map(|e| format!("mailto:{e}"))
                .or_else(|| o.get("sendTo").and_then(|s| s.get("imip")).and_then(Value::as_str).map(str::to_string));
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
