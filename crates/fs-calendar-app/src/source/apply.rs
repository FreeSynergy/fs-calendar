//! A form written back onto an event.
//!
//! Pattern: Command — only the fields that differ from what the form started with are applied, on top
//! of the event as it is stored now, so after a conflict a field the person left alone keeps what
//! someone else gave it. Everything else on the event stays untouched: properties the form does not
//! show, `extra`, participants, the recurrence rule and its overrides. A field the form does not know,
//! or a change to the times of a series, is refused rather than guessed at; nothing is changed then.

use chrono::Duration;
use fs_calendar_core::Event;
use fs_calendar_core::time::Zone;
use fs_components_workbenches::SourceError;
use fs_components_workbenches::calendar::{DraftField, ItemDraft};

use super::apply_clock::{keep_clock_times, timed_times};
use super::draft::{
    ALL_DAY, DESCRIPTION, END, LOCATION, START, TITLE, ZONE, is_series, when_texts,
};
use super::location::set_first_name;
use super::when::{Written, date_text, end_of, midnight, parse_in_range, start_value};
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

/// Has the person changed this field?
pub(super) fn changed(draft: &ItemDraft, key: &str) -> bool {
    draft.fields.iter().any(|f| f.key == key && f.changed())
}

/// The times: what the person changed, the rest as the event has it now. The end the person sees is the
/// end stored, so moving the start alone does not move the end. A duration is rewritten only if the
/// times the form shows would otherwise not be the ones stored, and then exactly.
fn apply_times(event: &mut Event, draft: &ItemDraft) -> Result<(), String> {
    let value = |key: &str| draft.value(key).unwrap_or_default();
    let (newest_start, newest_end) = when_texts(event)?;
    let end_before = end_of(event)?;
    if changed(draft, ZONE) {
        let name = value(ZONE).trim();
        if name.is_empty() {
            event.time_zone = None;
        } else {
            Zone::parse(name).map_err(|e| format!("\"{name}\" is not a time zone: {e}"))?;
            event.time_zone = Some(name.to_string());
        }
    }
    let toggled = changed(draft, ALL_DAY);
    let all_day = if toggled {
        value(ALL_DAY) == "true"
    } else {
        event.show_without_time == Some(true)
    };
    if !changed(draft, START) && !changed(draft, END) && !toggled {
        return if all_day || !changed(draft, ZONE) {
            Ok(())
        } else {
            keep_clock_times(event, end_before)
        };
    }
    let start_text = if changed(draft, START) {
        value(START)
    } else {
        &newest_start
    };
    let end_text = if changed(draft, END) {
        value(END)
    } else {
        &newest_end
    };
    let (start, end) = (parse_in_range(start_text)?, parse_in_range(end_text)?);
    if all_day {
        return all_day_times(event, start, end, toggled);
    }
    let (Written::Time(start), Written::Time(end)) = (start, end) else {
        let date = date_text(start.date());
        return Err(format!(
            "enter start and end with a time, like {date} 10:00"
        ));
    };
    timed_times(event, draft, start, end)
}

fn all_day_times(
    event: &mut Event,
    start: Written,
    end: Written,
    toggled: bool,
) -> Result<(), String> {
    let first = start.date();
    // An end at midnight is the end of the day before, not a part of its own day.
    let last = match end {
        Written::Time(time) if time == midnight(time.date()) && time.date() > first => {
            time.date() - Duration::days(1)
        }
        other => other.date(),
    };
    if last < first {
        return Err(say::END_BEFORE_START.to_string());
    }
    event.start = Some(start_value(midnight(first)));
    event.duration = Some(format!("P{}D", (last - first).num_days() + 1));
    event.show_without_time = Some(true);
    if toggled {
        // A day without a time is the same day wherever it is read.
        event.time_zone = None;
    }
    Ok(())
}
