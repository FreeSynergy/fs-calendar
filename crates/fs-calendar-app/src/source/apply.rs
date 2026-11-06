//! A form written back onto an event.
//!
//! Pattern: Command — only the fields that differ from what the form started with are applied. Everything
//! else on the event stays untouched: properties the form does not show, `extra`, participants, the
//! recurrence rule and its overrides. A field the form does not know, or a change to the times of a
//! series, is refused rather than guessed at; nothing is changed then.

use chrono::Duration;
use fs_calendar_core::Event;
use fs_calendar_core::time::Zone;
use fs_components_workbenches::SourceError;
use fs_components_workbenches::calendar::{DraftField, ItemDraft};

use super::draft::{ALL_DAY, DESCRIPTION, END, LOCATION, START, TITLE, ZONE, is_series};
use super::location::set_first_name;
use super::when::{Written, date_text, exact_duration, midnight, parse_written, start_value};
use crate::words::say;

/// Put what changed in `draft` onto `event`.
///
/// # Errors
/// [`SourceError`] with a reason for the person; `event` is then as it was.
pub fn apply_draft(event: &mut Event, draft: &ItemDraft) -> Result<(), SourceError> {
    let mut changed = event.clone();
    let mut times = false;
    for field in draft.fields.iter().filter(|f| f.changed()) {
        match field.key.as_str() {
            TITLE => {
                let title = field.value.trim();
                if title.is_empty() {
                    return Err(SourceError::failed(say::NEEDS_TITLE));
                }
                changed.title = Some(title.to_string());
            }
            LOCATION => set_first_name(&mut changed, field.value.trim()),
            DESCRIPTION => {
                changed.description = (!field.value.trim().is_empty()).then(|| field.value.clone());
            }
            START | END | ALL_DAY | ZONE => times = true,
            _ => return Err(unknown(field)),
        }
    }
    if times {
        if is_series(event) {
            return Err(SourceError::failed(say::SERIES_REFUSED));
        }
        apply_times(&mut changed, draft).map_err(SourceError::failed)?;
    }
    *event = changed;
    Ok(())
}

fn unknown(field: &DraftField) -> SourceError {
    SourceError::failed(format!("the form has an unknown field \"{}\"", field.key))
}

fn changed(draft: &ItemDraft, key: &str) -> bool {
    draft.fields.iter().any(|f| f.key == key && f.changed())
}

/// The times as the form holds them now. The end the person sees is the end stored: moving the start
/// alone does not move the end. A duration is rewritten only if start, end or all-day changed, and
/// exactly; with a zone change alone the same clock times stay.
fn apply_times(event: &mut Event, draft: &ItemDraft) -> Result<(), String> {
    let value = |key: &str| draft.value(key).unwrap_or_default();
    let all_day = value(ALL_DAY) == "true";
    if changed(draft, ZONE) {
        let name = value(ZONE).trim();
        if name.is_empty() {
            event.time_zone = None;
        } else {
            Zone::parse(name).map_err(|e| format!("\"{name}\" is not a time zone: {e}"))?;
            event.time_zone = Some(name.to_string());
        }
    }
    let span_changed = changed(draft, START) || changed(draft, END) || changed(draft, ALL_DAY);
    if !span_changed {
        return Ok(());
    }
    let start = parse_written(value(START))?;
    let end = parse_written(value(END))?;
    if all_day {
        let (first, last) = (start.date(), end.date());
        if last < first {
            return Err(say::END_BEFORE_START.to_string());
        }
        let days = (last - first).num_days() + 1;
        event.start = Some(start_value(midnight(first)));
        event.duration = Some(format!("P{days}D"));
        event.show_without_time = Some(true);
        if changed(draft, ALL_DAY) {
            // A day without a time is the same day wherever it is read.
            event.time_zone = None;
        }
        return Ok(());
    }
    let (Written::Time(start), Written::Time(end)) = (start, end) else {
        let date = date_text(start.date());
        return Err(format!(
            "enter start and end with a time, like {date} 10:00"
        ));
    };
    if end < start {
        return Err(say::END_BEFORE_START.to_string());
    }
    let seconds: Duration = match event.time_zone.as_deref().map(Zone::parse) {
        Some(Ok(zone)) => zone.to_utc(end) - zone.to_utc(start),
        _ => end - start,
    };
    event.start = Some(start_value(start));
    event.duration = Some(exact_duration(seconds.num_seconds().max(0)));
    if changed(draft, ALL_DAY) {
        event.show_without_time = None;
    }
    Ok(())
}
