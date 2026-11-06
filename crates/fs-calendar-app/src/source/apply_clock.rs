//! Writing clock times: a time must exist, an end must follow its start, a length is real time.
//!
//! Pattern: Command — helpers of `apply.rs`. A time the clocks skip is refused rather than stored as
//! something else than was typed.

use chrono::NaiveDateTime;
use fs_calendar_core::Event;
use fs_components_workbenches::calendar::ItemDraft;

use super::apply::changed;
use super::draft::{ALL_DAY, END, START, ZONE};
use super::when::{end_of, exact_duration, exists_in, start_of, start_value, time_text, zone_of};
use crate::words::say;

/// A time the clocks skip is refused: it would be stored as something else than was typed.
fn check_exists(event: &Event, time: NaiveDateTime) -> Result<(), String> {
    match zone_of(event) {
        Some(zone) if !exists_in(&zone, time) => Err(say::IN_GAP
            .replace("{time}", &time_text(time))
            .replace("{zone}", zone.name())),
        _ => Ok(()),
    }
}

fn length_between(
    event: &Event,
    start: NaiveDateTime,
    end: NaiveDateTime,
) -> Result<String, String> {
    let seconds = match zone_of(event) {
        Some(zone) => zone.to_utc(end) - zone.to_utc(start),
        None => end - start,
    }
    .num_seconds();
    if seconds < 0 {
        return Err(say::END_BEFORE_START.to_string());
    }
    Ok(exact_duration(seconds))
}

pub(super) fn timed_times(
    event: &mut Event,
    draft: &ItemDraft,
    start: NaiveDateTime,
    end: NaiveDateTime,
) -> Result<(), String> {
    let zone_changed = changed(draft, ZONE);
    if changed(draft, START) || zone_changed {
        check_exists(event, start)?;
    }
    if changed(draft, END) || zone_changed {
        check_exists(event, end)?;
    }
    if end < start {
        return Err(say::END_BEFORE_START.to_string());
    }
    event.duration = Some(length_between(event, start, end)?);
    if changed(draft, START) || changed(draft, ALL_DAY) {
        event.start = Some(start_value(start));
    }
    if changed(draft, ALL_DAY) {
        event.show_without_time = None;
    }
    Ok(())
}

/// Only the zone changed: start and end stay on the same clock. A length that crosses a clock change in
/// one zone and not in the other would move the end, so it is measured again, and only then.
pub(super) fn keep_clock_times(event: &mut Event, end_before: NaiveDateTime) -> Result<(), String> {
    let start = start_of(event)?;
    check_exists(event, start)?;
    // Where the clocks went back the end read earlier than the start: the length is what holds then.
    if end_before >= start && end_of(event)? != end_before {
        check_exists(event, end_before)?;
        event.duration = Some(length_between(event, start, end_before)?);
    }
    Ok(())
}
