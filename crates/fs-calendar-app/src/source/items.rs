//! Instances and tasks as items of the calendar view.
//!
//! Pattern: Adapter — the core speaks UTC and an event's own zone; the widget takes wall-clock times
//! in the zone shown. An all-day event and a floating one have no zone to convert from: their local
//! date and time are what is shown, wherever the person is. The rest are converted.

use chrono::{DateTime, Duration, Utc};
use fs_calendar_client::TaskLine;
use fs_calendar_core::Instance;
use fs_calendar_core::time::{Zone, parse_local};
use fs_components_workbenches::calendar::{CalendarItem, Day, Moment};

use super::marks::content_is_marked;
use super::when::{date_of, day_of, days_covered, duration_of, moment_of, start_of};
use crate::words::field;

/// Which category colour a thing takes.
const CANCELLED: u8 = 2;
const A_TASK: u8 = 1;

/// The item of `instance`, with `key`.
pub(super) fn item_of_instance(
    instance: &Instance,
    key: String,
    zone: &Zone,
    marked: bool,
) -> Result<CalendarItem, String> {
    let occurrence = &instance.occurrence;
    let event = &occurrence.event;
    let title = event.display_title();
    let start = start_of(event)?;
    let length = duration_of(event)?;
    let item = if event.show_without_time == Some(true) {
        let first = day_of(start.date());
        let covered = days_covered(length);
        CalendarItem::all_day(key, title, first, first.add_days(covered - 1))
    } else if event.time_zone.is_none() {
        let end = start + Duration::days(length.days) + Duration::seconds(length.secs);
        CalendarItem::timed(key, title, moment_of(start), moment_of(end))
    } else {
        let (from, to) = clock_span(occurrence.start_utc, occurrence.end_utc, zone);
        CalendarItem::timed(key, title, from, to)
    };
    let item = if event.status.as_deref() == Some("cancelled") {
        item.in_category(CANCELLED)
    } else {
        item
    };
    Ok(if marked {
        item.marked(field::SENSITIVE)
    } else {
        item
    })
}

/// Start and end on the clock of `zone`. Where the clocks go back an end can read earlier than its
/// start; the axis of the view is a clock, so there the real length is drawn from the start.
pub(super) fn clock_span(
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    zone: &Zone,
) -> (Moment, Moment) {
    let (from, to) = (zone.to_local(start), zone.to_local(end));
    let to = if to < from { from + (end - start) } else { to };
    (moment_of(from), moment_of(to))
}

/// The item of a task, at the time it is due: read through the tasks program's contract, never owned
/// here.
pub(super) fn item_of_task(entry: &TaskLine, zone: &Zone) -> Option<CalendarItem> {
    let content = &entry.content;
    let due = parse_local(content["due"].as_str()?).ok()?;
    let title = entry.name.clone();
    let all_day = content["showWithoutTime"].as_bool() == Some(true);
    let shown = match content["timeZone"].as_str().map(Zone::parse) {
        Some(Ok(own)) if !all_day => zone.to_local(own.to_utc(due)),
        _ => due,
    };
    let day = day_of(shown.date());
    let key = format!("task:{}@{day}", entry.id);
    let item = if all_day {
        CalendarItem::all_day(key, title, day, day)
    } else {
        let at = moment_of(shown);
        CalendarItem::timed(key, title, at, at)
    };
    let item = item.in_category(A_TASK).marked(field::TASK);
    Some(if content_is_marked(content) {
        item.marked(field::SENSITIVE)
    } else {
        item
    })
}

/// Does `item` have a part on the days `first` through `last`? Goes by the wall clock.
pub(super) fn is_within(item: &CalendarItem, first: Day, last: Day) -> bool {
    let from = Moment::start_of(first);
    if item.start.day > last {
        return false;
    }
    item.end > from || (item.start == item.end && item.start >= from)
}

/// The instants of the window to ask the core for: a wide margin around the days, because the core
/// places all-day and floating events by its own floating zone, not the one shown. Bounded by the days
/// asked for, so an endless series is never expanded further than that.
pub(super) fn window(first: Day, last: Day, zone: &Zone) -> (DateTime<Utc>, DateTime<Utc>) {
    let margin = Duration::days(2);
    let at = |day: Day| zone.to_utc(super::when::midnight(date_of(day)));
    (at(first) - margin, at(last.add_days(1)) + margin)
}
