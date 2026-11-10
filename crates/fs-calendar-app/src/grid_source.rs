//! The events and the due tasks as items of the calendar grid.
//!
//! Pattern: Adapter (`Calendar` to `GridSource`) — asks the core for the instances of the shown days (`agenda`: events
//! and the tasks due, the tasks through the tasks interface), turns UTC into the shown zone and the JSCalendar
//! objects into the plain items the component takes. The grid knows no JSCalendar type, this file knows no look.

use std::sync::Arc;

use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime, Timelike};
use fs_calendar_core::time::parse_local;
use fs_calendar_core::{Calendar, DueTask, Instance, Zone};
use fs_components_calendar_grid::{Day, GridItem, GridSource, Moment, Tone};
use tokio::runtime::Handle;

use crate::backend::instance_id;

/// Items for the grid, read from the calendar when the grid asks.
pub struct CalendarSource {
    calendar: Arc<dyn Calendar>,
    handle: Handle,
    zone: Zone,
}

impl CalendarSource {
    /// Items of `calendar`, times shown in `zone`.
    #[must_use]
    pub fn new(calendar: Arc<dyn Calendar>, handle: Handle, zone: Zone) -> Self {
        Self {
            calendar,
            handle,
            zone,
        }
    }
}

fn naive(day: Day) -> NaiveDateTime {
    let (y, m, d) = day.ymd();
    NaiveDate::from_ymd_opt(y, u32::from(m), u32::from(d))
        .unwrap_or(NaiveDate::MIN)
        .and_hms_opt(0, 0, 0)
        .unwrap_or_default()
}

fn day_of(t: NaiveDateTime) -> Day {
    Day::from_ymd(
        t.year(),
        u8::try_from(t.month()).unwrap_or(1),
        u8::try_from(t.day()).unwrap_or(1),
    )
}

fn moment(t: NaiveDateTime) -> Moment {
    Moment::new(
        day_of(t),
        u16::try_from(t.hour() * 60 + t.minute()).unwrap_or(0),
    )
}

fn event_item(zone: &Zone, i: &Instance) -> GridItem {
    let (event, o) = (&i.occurrence.event, &i.occurrence);
    let key = instance_id(i);
    let (mut start, mut end) = (zone.to_local(o.start_utc), zone.to_local(o.end_utc));
    if event.show_without_time == Some(true) {
        // A day item is a date, not an instant: it stays on the days the event names, whatever zone is shown.
        if let Some(local) = event.start.as_deref().and_then(|t| parse_local(t).ok()) {
            (start, end) = (local, local + (o.end_utc - o.start_utc));
        }
    }
    let tone = match event.status.as_deref() {
        Some("cancelled") => Tone::Error,
        Some("tentative") => Tone::Neutral,
        _ => Tone::Accent,
    };
    let title = event.display_title();
    if event.show_without_time == Some(true) {
        let last = (end - Duration::seconds(1)).max(start);
        return GridItem::all_day(&key, &title, day_of(start), day_of(last)).tone(tone);
    }
    let (from, mut to) = (moment(start), moment(end));
    if to.day == from.day && to.minute <= from.minute {
        to.minute = (from.minute + 15).min(1439);
    }
    GridItem::timed(&key, &title, from, to).tone(tone)
}

/// A task shows on the day it is due, as a day item.
fn task_item(t: &DueTask) -> Option<GridItem> {
    let due = t.jscalendar.get("due")?.as_str()?;
    let day = NaiveDateTime::parse_from_str(due, "%Y-%m-%dT%H:%M:%S")
        .ok()
        .map(day_of)?;
    let title = t
        .jscalendar
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("(task)");
    Some(GridItem::all_day(&format!("task:{}", t.id), title, day, day).tone(Tone::Success))
}

impl GridSource for CalendarSource {
    fn items(&self, first: Day, last: Day) -> Vec<GridItem> {
        let from = self.zone.to_utc(naive(first));
        let to = self.zone.to_utc(naive(last.add_days(1)));
        match self.handle.block_on(self.calendar.agenda(from, to)) {
            Ok(agenda) => agenda
                .events
                .iter()
                .map(|i| event_item(&self.zone, i))
                .chain(agenda.tasks.iter().filter_map(task_item))
                .collect(),
            Err(e) => {
                eprintln!("fs-calendar: the calendar could not be read: {e}");
                Vec::new()
            }
        }
    }
}
