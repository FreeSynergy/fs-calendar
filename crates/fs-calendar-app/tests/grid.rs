//! The calendar as items of the grid: zones, series, all-day events.
//!
//! Pattern: none (tests).
#![allow(clippy::pedantic)]

use std::sync::Arc;

use fs_calendar_app::CalendarSource;
use fs_calendar_core::{Calendar, CalendarRepo, Event, Zone};
use fs_components_calendar_grid::{Day, GridSource, Moment};
use fs_objects::{Store, TursoStore};

const ICS: &str = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//x//y//EN\r\nBEGIN:VEVENT\r\nUID:ev-1\r\nDTSTAMP:20261101T000000Z\r\nDTSTART;TZID=Europe/Berlin:20261124T100000\r\nDTEND;TZID=Europe/Berlin:20261124T113000\r\nSUMMARY:Jour fixe\r\nRRULE:FREQ=WEEKLY;COUNT=4;BYDAY=TU\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";

#[test]
fn events_become_items_in_the_shown_zone() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let store: Arc<dyn Store> = Arc::new(rt.block_on(TursoStore::open_in_memory()).unwrap());
    let repo = Arc::new(CalendarRepo::new(store, "https://local.invalid/u"));
    rt.block_on(repo.import_ics(ICS)).unwrap();
    let mut day = Event::new("Holiday", "2026-11-26T00:00:00", "Europe/Berlin", "P2D");
    day.show_without_time = Some(true);
    rt.block_on(repo.create(day)).unwrap();
    let source = CalendarSource::new(
        repo,
        rt.handle().clone(),
        Zone::parse("Asia/Tokyo").unwrap(),
    );
    let items = source.items(Day::from_ymd(2026, 11, 20), Day::from_ymd(2026, 12, 3));
    let series: Vec<_> = items.iter().filter(|i| i.title == "Jour fixe").collect();
    assert_eq!(series.len(), 2, "{items:?}");
    // 10:00 in Berlin is 18:00 in Tokyo
    assert_eq!(
        series[0].start,
        Moment::new(Day::from_ymd(2026, 11, 24), 18 * 60)
    );
    assert_eq!(
        series[0].end,
        Moment::new(Day::from_ymd(2026, 11, 24), 19 * 60 + 30)
    );
    assert!(series[0].key.starts_with("urn:uuid:") && series[0].key.contains('#'));
    let holiday = items.iter().find(|i| i.title == "Holiday").unwrap();
    assert!(holiday.all_day);
    // a day item stays on its days in any zone
    assert_eq!(holiday.start.day, Day::from_ymd(2026, 11, 26));
    assert_eq!(holiday.last_day(), Day::from_ymd(2026, 11, 27));
}
