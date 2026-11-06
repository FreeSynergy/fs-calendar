//! Times of the form and of the calendar.
//!
//! Pattern: pure functions over the core's time types — text to times and back, the end of an event in
//! its own zone, a duration from two local times, and a local time as a moment of the widget.

use std::fmt::Write as _;

use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime, Timelike};
use fs_calendar_core::Event;
use fs_calendar_core::time::{Dur, Zone, fmt_local, parse_local};
use fs_components_workbenches::calendar::{Day, FIRST_YEAR, LAST_YEAR, Moment};

/// More days than this is no length of an event: every year the calendar shows, and then some.
const LONGEST_DAYS: i64 = 4_000_000;

/// What the person wrote: a date, or a date with a time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Written {
    Date(NaiveDate),
    Time(NaiveDateTime),
}

impl Written {
    pub(super) fn date(self) -> NaiveDate {
        match self {
            Self::Date(date) => date,
            Self::Time(time) => time.date(),
        }
    }
}

/// `YYYY-MM-DD`, with ` HH:MM` or `THH:MM` and perhaps `:SS`.
pub(super) fn parse_written(text: &str) -> Result<Written, String> {
    let text = text.trim();
    let bad =
        || format!("\"{text}\" is not a date like 2026-11-24 or a time like 2026-11-24 10:00");
    let normal = text.replacen(' ', "T", 1);
    for format in ["%Y-%m-%dT%H:%M:%S", "%Y-%m-%dT%H:%M"] {
        if let Ok(time) = NaiveDateTime::parse_from_str(&normal, format) {
            return Ok(Written::Time(time));
        }
    }
    NaiveDate::parse_from_str(text, "%Y-%m-%d")
        .map(Written::Date)
        .map_err(|_| bad())
}

/// [`parse_written`], within the years the calendar shows; anything further is refused before it
/// reaches a calculation that cannot hold it.
pub(super) fn parse_in_range(text: &str) -> Result<Written, String> {
    let written = parse_written(text)?;
    if (FIRST_YEAR..=LAST_YEAR).contains(&written.date().year()) {
        Ok(written)
    } else {
        Err(format!(
            "\"{}\" is outside the years {FIRST_YEAR} to {LAST_YEAR}, which is what the calendar shows",
            text.trim()
        ))
    }
}

/// Does this clock time happen in `zone`? Not in the hour skipped when the clocks jump forward.
pub(super) fn exists_in(zone: &Zone, time: NaiveDateTime) -> bool {
    zone.to_local(zone.to_utc(time)) == time
}

/// The zone of `event`, if it has one that is known.
pub(super) fn zone_of(event: &Event) -> Option<Zone> {
    event
        .time_zone
        .as_deref()
        .and_then(|name| Zone::parse(name).ok())
}

/// A date as the form shows it.
pub(super) fn date_text(date: NaiveDate) -> String {
    date.format("%Y-%m-%d").to_string()
}

/// A time as the form shows it; seconds only if there are any.
pub(super) fn time_text(time: NaiveDateTime) -> String {
    if time.second() == 0 {
        time.format("%Y-%m-%d %H:%M").to_string()
    } else {
        time.format("%Y-%m-%d %H:%M:%S").to_string()
    }
}

/// `date` at midnight.
pub(super) fn midnight(date: NaiveDate) -> NaiveDateTime {
    date.and_hms_opt(0, 0, 0).unwrap_or_default()
}

/// A JSCalendar start for `time`.
pub(super) fn start_value(time: NaiveDateTime) -> String {
    fmt_local(time)
}

/// The local start of `event`.
pub(super) fn start_of(event: &Event) -> Result<NaiveDateTime, String> {
    parse_local(event.start.as_deref().unwrap_or_default()).map_err(|e| e.to_string())
}

/// How long `event` is.
pub(super) fn duration_of(event: &Event) -> Result<Dur, String> {
    let length = event
        .duration
        .as_deref()
        .map_or(Ok(Dur::default()), Dur::parse)
        .map_err(|e| e.to_string())?;
    if length.days.abs() > LONGEST_DAYS || length.secs.abs() / 86_400 > LONGEST_DAYS {
        return Err(format!(
            "a length of {} is too long",
            event.duration.as_deref().unwrap_or_default()
        ));
    }
    Ok(length)
}

/// The end of `event` as a local time in its own zone; a floating one is counted on the wall clock.
pub(super) fn end_of(event: &Event) -> Result<NaiveDateTime, String> {
    let start = start_of(event)?;
    let length = duration_of(event)?;
    Ok(
        match event.time_zone.as_deref().map(Zone::parse).transpose() {
            Ok(Some(zone)) => zone.to_local(length.end_utc(&zone, start)),
            _ => start + Duration::days(length.days) + Duration::seconds(length.secs),
        },
    )
}

/// How many days an event without a time covers: at least one.
pub(super) fn days_covered(length: Dur) -> i64 {
    let whole_days = (length.secs + 86_399).div_euclid(86_400);
    (length.days + whole_days).max(1)
}

/// `seconds` as an exact ISO 8601 duration.
pub(super) fn exact_duration(seconds: i64) -> String {
    let (hours, minutes, rest) = (seconds / 3600, seconds % 3600 / 60, seconds % 60);
    let mut text = String::from("PT");
    if hours > 0 {
        let _ = write!(text, "{hours}H");
    }
    if minutes > 0 {
        let _ = write!(text, "{minutes}M");
    }
    if rest > 0 || text == "PT" {
        let _ = write!(text, "{rest}S");
    }
    text
}

/// A calendar day of `date`.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub(super) fn day_of(date: NaiveDate) -> Day {
    Day::new(date.year(), date.month() as u8, date.day() as u8)
        .unwrap_or_else(|| Day::from_number(0))
}

/// A moment of `time`.
#[allow(clippy::cast_possible_truncation)]
pub(super) fn moment_of(time: NaiveDateTime) -> Moment {
    Moment::at(
        day_of(time.date()),
        time.hour() as u16,
        time.minute() as u16,
    )
}

/// The date of a calendar day.
pub(super) fn date_of(day: Day) -> NaiveDate {
    NaiveDate::from_ymd_opt(day.year(), u32::from(day.month()), u32::from(day.day()))
        .unwrap_or_default()
}
