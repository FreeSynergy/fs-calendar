//! All-day events, time zones, DST days, events over midnight and over months, week start, far future.
//!
//! Pattern: Table tests over the real core: what is stored and what the calendar view is given.

mod common;

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use common::{World, d, sheet_text};
use fs_calendar_app::CalendarAdapter;
use fs_calendar_core::{Agenda, Calendar, CalendarRepo, Capabilities, Event, Instance, Zone};
use fs_components_workbenches::calendar::{CalendarSource, Moment, Shown, ViewRange};
use fs_store::{ObjectId, StoredObject};
use serde_json::json;

fn event(title: &str, start: &str, zone: Option<&str>, duration: &str) -> Event {
    let mut e = Event::new(title, start, zone.unwrap_or("Etc/UTC"), duration);
    e.time_zone = zone.map(str::to_string);
    e
}

fn all_day(title: &str, start: &str, duration: &str) -> Event {
    let mut e = event(title, start, None, duration);
    e.show_without_time = Some(true);
    e
}

fn m(day: &str, hour: u16, minute: u16) -> Moment {
    Moment::at(d(day), hour, minute)
}

#[test]
fn an_all_day_event_stays_on_its_day_in_any_zone_shown() {
    for zone in ["Pacific/Auckland", "Pacific/Honolulu", "Europe/Berlin"] {
        let w = World::shown_in(zone);
        w.add(all_day("Holiday", "2026-11-24T00:00:00", "P1D"));
        let item = w.item("2026-11-24", "Holiday");
        assert!(item.all_day);
        assert_eq!(item.start, Moment::start_of(d("2026-11-24")), "{zone}");
        assert!(w.items("2026-11-23", "2026-11-23").is_empty(), "{zone}");
        assert!(w.items("2026-11-25", "2026-11-25").is_empty(), "{zone}");
    }
}

#[test]
fn a_many_day_all_day_event_crosses_a_month_and_the_form_says_its_last_day() {
    let w = World::berlin();
    let key = w.add(all_day("Trip", "2026-11-29T00:00:00", "P3D"));
    for day in ["2026-11-29", "2026-11-30", "2026-12-01"] {
        assert_eq!(w.items(day, day).len(), 1, "{day}");
    }
    assert!(w.items("2026-12-02", "2026-12-02").is_empty());
    let draft = w.adapter.draft(&key).unwrap();
    assert_eq!(draft.value("start"), Some("2026-11-29"));
    assert_eq!(draft.value("end"), Some("2026-12-01"));
    assert_eq!(draft.value("all_day"), Some("true"));
    assert_eq!(draft.value("time_zone"), Some(""));
}

#[test]
fn an_imported_all_day_event_is_all_day() {
    let w = World::berlin();
    let ics = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//x//y//EN\r\nBEGIN:VEVENT\r\nUID:ad-1\r\nDTSTAMP:20261101T000000Z\r\nDTSTART;VALUE=DATE:20261124\r\nDTEND;VALUE=DATE:20261126\r\nSUMMARY:Imported days\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
    w.runtime.block_on(w.repo.import_ics(ics)).unwrap();
    let item = w.item("2026-11-25", "Imported days");
    assert!(item.all_day, "{item:?}");
    assert_eq!(item.start.day, d("2026-11-24"));
    assert!(w.items("2026-11-26", "2026-11-26").is_empty());
}

#[test]
fn turning_a_timed_event_into_all_day_and_back_is_what_the_form_says() {
    let w = World::berlin();
    let key = w.add(event(
        "Flip",
        "2026-11-24T22:00:00",
        Some("Asia/Tokyo"),
        "PT3H",
    ));
    let mut s = w.state();
    s.select(&key);
    s.begin_edit();
    s.toggle_field("all_day");
    s.save();
    assert!(s.editing().is_none(), "{:?}", s.notice());
    let stored = w.stored(&key).content;
    assert_eq!(stored["showWithoutTime"], json!(true));
    assert_eq!(stored["start"], json!("2026-11-24T00:00:00"));
    assert_eq!(
        stored["duration"],
        json!("P2D"),
        "22:00 to 01:00 touches two days"
    );
    assert_eq!(stored.get("timeZone"), None);
    assert!(w.item("2026-11-25", "Flip").all_day);

    s.begin_edit();
    s.toggle_field("all_day");
    s.save();
    assert!(
        s.editing()
            .unwrap()
            .error
            .as_deref()
            .unwrap()
            .contains("with a time")
    );
    s.set_field("start", "2026-11-24 10:00");
    s.set_field("end", "2026-11-24 11:00");
    s.set_field("time_zone", "Europe/Berlin");
    s.save();
    assert!(s.editing().is_none(), "{:?}", s.notice());
    let stored = w.stored(&key).content;
    assert_eq!(stored.get("showWithoutTime"), None);
    assert_eq!(stored["start"], json!("2026-11-24T10:00:00"));
    assert_eq!(stored["duration"], json!("PT1H"));
    assert!(!w.item("2026-11-24", "Flip").all_day);
}

#[test]
fn an_event_in_another_zone_is_shown_on_the_clock_of_the_zone_shown() {
    let w = World::berlin();
    let key = w.add(event(
        "Tokyo call",
        "2026-11-25T01:00:00",
        Some("Asia/Tokyo"),
        "PT1H",
    ));
    let item = w.item("2026-11-24", "Tokyo call");
    assert_eq!(
        (item.start, item.end),
        (m("2026-11-24", 17, 0), m("2026-11-24", 18, 0))
    );
    assert!(
        w.items("2026-11-25", "2026-11-25").is_empty(),
        "the 25th only in Tokyo"
    );
    let mut s = w.state();
    s.select(&key);
    let text = sheet_text(&s);
    assert!(text.contains("Start: 2026-11-25 01:00"), "{text}");
    assert!(
        text.contains("In the zone shown: 2026-11-24 17:00"),
        "{text}"
    );
    s.begin_edit();
    assert_eq!(
        s.editing().unwrap().draft.value("start"),
        Some("2026-11-25 01:00"),
        "its own zone"
    );
}

#[test]
fn a_floating_event_keeps_its_clock_in_any_zone_shown() {
    for zone in ["Asia/Tokyo", "America/New_York"] {
        let w = World::shown_in(zone);
        w.add(event("Coffee", "2026-11-24T09:00:00", None, "PT30M"));
        let item = w.item("2026-11-24", "Coffee");
        assert_eq!(item.start, m("2026-11-24", 9, 0), "{zone}");
    }
}

#[test]
fn an_event_over_midnight_is_on_both_days_and_over_a_month_end_too() {
    let w = World::berlin();
    w.add(event(
        "Night",
        "2026-11-30T22:00:00",
        Some("Europe/Berlin"),
        "PT4H",
    ));
    let night = w.item("2026-11-30", "Night");
    assert_eq!(night.end, m("2026-12-01", 2, 0));
    assert_eq!(w.item("2026-12-01", "Night").key, night.key);
    assert!(w.items("2026-12-02", "2026-12-02").is_empty());
}

#[test]
fn a_day_when_the_clocks_change_keeps_clock_times_and_real_lengths() {
    let w = World::berlin();
    // 2026-10-25: at 03:00 summer time clocks go back to 02:00. 22:00 to 04:00 is seven real hours.
    let key = w.add(event(
        "Autumn",
        "2026-10-24T22:00:00",
        Some("Europe/Berlin"),
        "PT7H",
    ));
    let item = w.item("2026-10-25", "Autumn");
    assert_eq!(item.end, m("2026-10-25", 4, 0));
    let draft = w.adapter.draft(&key).unwrap();
    assert_eq!(draft.value("end"), Some("2026-10-25 04:00"));
    let mut s = w.state();
    s.select(&key);
    s.begin_edit();
    s.set_field("start", "2026-10-24 23:00");
    s.save();
    assert!(s.editing().is_none(), "{:?}", s.notice());
    assert_eq!(
        w.stored(&key).content["duration"],
        json!("PT6H"),
        "real hours to the same clock end"
    );
    assert_eq!(w.item("2026-10-25", "Autumn").end, m("2026-10-25", 4, 0));

    // 2027-03-28: at 02:00 clocks jump to 03:00. A nominal day is 23 real hours and ends on the clock.
    let spring = w.add(event(
        "Spring",
        "2027-03-27T12:00:00",
        Some("Europe/Berlin"),
        "P1D",
    ));
    assert_eq!(w.item("2027-03-28", "Spring").end, m("2027-03-28", 12, 0));
    assert_eq!(
        w.adapter.draft(&spring).unwrap().value("end"),
        Some("2027-03-28 12:00")
    );

    // A daily event keeps its clock time across the change.
    let mut daily = event(
        "Daily",
        "2026-10-24T09:00:00",
        Some("Europe/Berlin"),
        "PT1H",
    );
    daily.recurrence_rule =
        serde_json::from_value(json!({"frequency": "daily", "count": 3})).unwrap();
    w.add(daily);
    for day in ["2026-10-24", "2026-10-25", "2026-10-26"] {
        assert_eq!(w.item(day, "Daily").start, m(day, 9, 0), "{day}");
    }
}

#[test]
fn a_series_in_another_zone_moves_across_the_days_of_the_zone_shown() {
    let w = World::shown_in("Pacific/Auckland");
    let mut weekly = event(
        "Weekly",
        "2026-11-03T20:00:00",
        Some("America/New_York"),
        "PT1H",
    );
    weekly.recurrence_rule = serde_json::from_value(json!({"frequency": "weekly"})).unwrap();
    w.add(weekly);
    let item = w.item("2026-11-04", "Weekly");
    assert_eq!(
        item.start,
        m("2026-11-04", 14, 0),
        "20:00 EST is 01:00 UTC is 14:00 NZDT"
    );
}

#[test]
fn the_week_start_shows_the_right_days() {
    let w = World::berlin();
    w.add(event(
        "Sunday",
        "2026-11-01T10:00:00",
        Some("Europe/Berlin"),
        "PT1H",
    ));
    let adapter = CalendarAdapter::new(
        w.repo.clone(),
        w.runtime.handle().clone(),
        Zone::parse("Europe/Berlin").unwrap(),
    )
    .frozen_at(m("2026-11-06", 12, 0))
    .week_start(6);
    let s = fs_components_workbenches::calendar::CalendarState::new(
        Arc::new(adapter),
        fs_calendar_app::labels(),
        fs_components_workbenches::calendar::Variant::Full,
    );
    assert_eq!(s.week_start(), 6);
    let mut s = s;
    s.set_view(ViewRange::Week);
    assert_eq!(s.span().0, d("2026-11-01"));
    assert_eq!(s.items().len(), 1, "that Sunday is in this week");
    let mut monday = w.state();
    monday.set_view(ViewRange::Week);
    assert!(
        monday.items().is_empty(),
        "with Monday first it is last week's"
    );
}

#[test]
fn a_far_future_date_shows_an_endless_series_and_a_single_event() {
    let w = World::berlin();
    let mut weekly = event(
        "Endless",
        "2026-11-03T10:00:00",
        Some("Europe/Berlin"),
        "PT1H",
    );
    weekly.recurrence_rule = serde_json::from_value(json!({"frequency": "weekly"})).unwrap();
    let key = w.add(weekly);
    w.add(event(
        "One far",
        "2999-06-15T10:00:00",
        Some("Europe/Berlin"),
        "PT1H",
    ));
    let mut s = w.state();
    s.go_to(d("2999-06-10"));
    assert_eq!(s.problem(), None);
    assert!(s.items().iter().any(|i| i.title == "One far"));
    let far: Vec<_> = s.items().iter().filter(|i| i.title == "Endless").collect();
    assert!(far.len() >= 4, "{far:?}");
    let first = far[0].key.clone();
    assert!(first.starts_with(&key));
    s.select(&first);
    assert!(matches!(s.shown(), Shown::Item { .. }), "{:?}", s.shown());
    s.go_to(d("2026-11-06"));
    assert!(s.items().iter().any(|i| i.title == "Endless"));
}

/// The real core, with a note of the windows it is asked about.
struct Recording {
    core: Arc<CalendarRepo>,
    asked: Mutex<Vec<(DateTime<Utc>, DateTime<Utc>)>>,
}

#[async_trait]
impl Calendar for Recording {
    fn capabilities(&self) -> Capabilities {
        Calendar::capabilities(self.core.as_ref())
    }
    async fn get(&self, id: ObjectId) -> fs_calendar_core::Result<Option<StoredObject>> {
        Calendar::get(self.core.as_ref(), id).await
    }
    async fn get_event(&self, id: ObjectId) -> fs_calendar_core::Result<Option<Event>> {
        Calendar::get_event(self.core.as_ref(), id).await
    }
    async fn get_by_uid(&self, uid: &str) -> fs_calendar_core::Result<Option<StoredObject>> {
        Calendar::get_by_uid(self.core.as_ref(), uid).await
    }
    async fn list_events(&self) -> fs_calendar_core::Result<Vec<StoredObject>> {
        Calendar::list_events(self.core.as_ref()).await
    }
    async fn occurrences(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> fs_calendar_core::Result<Vec<Instance>> {
        self.asked.lock().unwrap().push((from, to));
        Calendar::occurrences(self.core.as_ref(), from, to).await
    }
    async fn agenda(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> fs_calendar_core::Result<Agenda> {
        self.asked.lock().unwrap().push((from, to));
        Calendar::agenda(self.core.as_ref(), from, to).await
    }
}

#[test]
fn occurrences_are_only_ever_asked_for_about_the_days_a_view_shows() {
    let w = World::berlin();
    let mut daily = event(
        "Endless",
        "2020-01-01T10:00:00",
        Some("Europe/Berlin"),
        "PT1H",
    );
    daily.recurrence_rule = serde_json::from_value(json!({"frequency": "daily"})).unwrap();
    w.add(daily);
    let recording = Arc::new(Recording {
        core: w.repo.clone(),
        asked: Mutex::new(Vec::new()),
    });
    let adapter = CalendarAdapter::new(
        recording.clone(),
        w.runtime.handle().clone(),
        Zone::parse("Europe/Berlin").unwrap(),
    )
    .frozen_at(m("2026-11-06", 12, 0));
    let mut s = fs_components_workbenches::calendar::CalendarState::new(
        Arc::new(adapter),
        fs_calendar_app::labels(),
        fs_components_workbenches::calendar::Variant::Full,
    );
    for view in [
        ViewRange::Month,
        ViewRange::Week,
        ViewRange::Day,
        ViewRange::Agenda,
    ] {
        s.set_view(view);
        s.next();
        s.previous();
    }
    let first = s.items()[0].key.clone();
    s.select(&first);
    let asked = recording.asked.lock().unwrap();
    assert!(asked.len() >= 8);
    for (from, to) in asked.iter() {
        let days = (*to - *from).num_days();
        assert!(days <= 47, "{from} to {to}: {days} days");
    }
}

#[test]
fn a_new_event_in_a_zone_that_is_not_the_one_shown_lands_on_the_right_day() {
    let w = World::berlin();
    let mut s = w.state();
    s.begin_create();
    s.set_field("title", "Abroad");
    s.set_field("start", "2027-02-01 00:30");
    s.set_field("end", "2027-02-01 01:30");
    s.set_field("time_zone", "Asia/Tokyo");
    s.save();
    assert!(s.editing().is_none(), "{:?}", s.notice());
    assert_eq!(s.anchor(), d("2027-01-31"), "it is still the 31st here");
}

#[test]
fn the_far_end_of_the_calendar_answers_for_an_endless_daily_series() {
    let w = World::berlin();
    let mut daily = event(
        "Daily",
        "2020-01-01T10:00:00",
        Some("Europe/Berlin"),
        "PT1H",
    );
    daily.recurrence_rule = serde_json::from_value(json!({"frequency": "daily"})).unwrap();
    w.add(daily);
    let started = std::time::Instant::now();
    let items = w.items("8999-12-01", "8999-12-07");
    eprintln!(
        "7 days of year 8999: {} items in {:?}",
        items.len(),
        started.elapsed()
    );
    let started = std::time::Instant::now();
    let near = w.items("2026-12-01", "2026-12-07");
    eprintln!(
        "7 days of year 2026: {} items in {:?}",
        near.len(),
        started.elapsed()
    );
    assert_eq!(near.len(), 7);
    assert_eq!(items.len(), 7, "the series does not stop in the far future");
}
