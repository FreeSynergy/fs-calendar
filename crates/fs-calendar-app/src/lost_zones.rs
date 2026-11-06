//! Events whose time zone an import could not keep.
//!
//! Pattern: Specification over the text of the file — until the core says so itself, an event whose
//! `TZID` names no known zone and that was stored without a zone is found here and named, because it
//! now means another time.

use fs_calendar_core::{Calendar, Zone};
use tokio::runtime::Handle;

/// Lines with their folded continuations joined (RFC 5545).
fn unfolded(text: &str) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for line in text.lines() {
        match (line.strip_prefix([' ', '\t']), lines.last_mut()) {
            (Some(rest), Some(last)) => last.push_str(rest),
            _ => lines.push(line.to_string()),
        }
    }
    lines
}

/// The `TZID` of a `DTSTART` or `DTEND` line.
fn tzid_of(line: &str) -> Option<&str> {
    let (head, _) = line.split_once(':')?;
    let upper = head.to_ascii_uppercase();
    if !upper.starts_with("DTSTART") && !upper.starts_with("DTEND") {
        return None;
    }
    let at = upper.find("TZID=")? + "TZID=".len();
    let rest = &head[at..];
    Some(rest.split(';').next()?.trim_matches('"'))
}

/// `uid`s and names of the events in `text` with a `TZID` no zone answers to.
fn with_unknown_zone(text: &str) -> Vec<(String, String)> {
    let mut found = Vec::new();
    let (mut inside, mut uid, mut summary, mut unknown) =
        (false, String::new(), String::new(), false);
    for line in unfolded(text) {
        let upper = line.to_ascii_uppercase();
        if upper.starts_with("BEGIN:VEVENT") {
            (inside, unknown) = (true, false);
            uid.clear();
            summary.clear();
        } else if upper.starts_with("END:VEVENT") {
            if inside && unknown {
                found.push((uid.clone(), summary.clone()));
            }
            inside = false;
        } else if inside {
            if let Some(value) = line.strip_prefix("UID:") {
                value.clone_into(&mut uid);
            } else if let Some(value) = line.strip_prefix("SUMMARY:") {
                value.clone_into(&mut summary);
            } else if tzid_of(&line).is_some_and(|tzid| Zone::parse(tzid).is_err()) {
                unknown = true;
            }
        }
    }
    found
}

/// The events of this file that are stored without the zone the file gave them, in words.
pub(crate) fn lost_zones(calendar: &dyn Calendar, runtime: &Handle, text: &str) -> Vec<String> {
    with_unknown_zone(text)
        .into_iter()
        .filter(|(uid, _)| {
            let stored = runtime.block_on(calendar.get_by_uid(uid)).ok().flatten();
            stored.is_some_and(|object| object.content.get("timeZone").is_none())
        })
        .map(|(uid, summary)| format!("{summary} ({uid})"))
        .collect()
}
