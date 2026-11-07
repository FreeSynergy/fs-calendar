//! How much the core is asked for: never more than a view shows, an endless series or not.
//!
//! Decorator over the real client, which notes the windows it is asked about.

mod common;

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use common::{World, d};
use fs_calendar_app::CalendarAdapter;
use fs_calendar_client::RpcCalendar;
use fs_calendar_core::{Calendar, Capabilities, Event, Instance, Zone};
use fs_components_workbenches::calendar::{Moment, ViewRange};
use fs_objects::{ObjectId, StoredObject};
use serde_json::json;

fn event(title: &str, start: &str, zone: Option<&str>, duration: &str) -> Event {
    let mut e = Event::new(title, start, zone.unwrap_or("Etc/UTC"), duration);
    e.time_zone = zone.map(str::to_string);
    e
}

fn m(day: &str, hour: u16, minute: u16) -> Moment {
    Moment::at(d(day), hour, minute)
}

/// The real client, with a note of the windows it is asked about.
struct Recording {
    core: Arc<RpcCalendar>,
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
        core: w.client.clone(),
        asked: Mutex::new(Vec::new()),
    });
    let adapter = CalendarAdapter::new(
        recording.clone(),
        w.client.clone(),
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
fn an_event_every_five_minutes_is_shown_whole_though_the_core_cuts_the_window() {
    let w = World::berlin();
    let mut minutely = event(
        "Every minute",
        "2026-10-01T00:00:00",
        Some("Europe/Berlin"),
        "PT1M",
    );
    minutely.recurrence_rule =
        serde_json::from_value(json!({"frequency": "minutely", "interval": 5})).unwrap();
    w.add(minutely);
    let started = std::time::Instant::now();
    let read = fs_components_workbenches::calendar::CalendarSource::items(
        w.adapter.as_ref(),
        d("2026-10-26"),
        d("2026-12-06"),
    )
    .unwrap();
    eprintln!(
        "{} items in {:?}: {:?}",
        read.items.len(),
        started.elapsed(),
        read.problems
    );
    // The core cuts such a window at 10 000 instances of one event and the window is read again event by
    // event (the fallback); a rule that gives 65 535 starts in a view can no longer be written.
    assert_eq!(read.items.len(), 12096, "{:?}", read.problems);
    assert!(read.problems.is_empty(), "{:?}", read.problems);
    let week = fs_components_workbenches::calendar::CalendarSource::items(
        w.adapter.as_ref(),
        d("2026-11-02"),
        d("2026-11-03"),
    )
    .unwrap();
    assert!(
        week.problems.is_empty(),
        "two days are not cut: {:?}",
        week.problems
    );
}
