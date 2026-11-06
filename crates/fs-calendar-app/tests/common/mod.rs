//! Shared helpers of the tests: the real core over an in-memory store, behind the real adapter.
//!
//! Pattern: Facade over the set-up. Nothing here pretends to be the core.
#![allow(dead_code)] // each test file uses a different subset

use std::path::PathBuf;
use std::sync::Arc;

use fs_calendar_app::CalendarAdapter;
use fs_calendar_core::{Calendar, CalendarRepo, Event, Zone};
use fs_components_workbenches::calendar::{
    CalendarItem, CalendarLabels, CalendarSource, CalendarState, Day, Moment, Shown, Variant,
};
use fs_store::{ObjectId, Store, StoredObject, TursoStore};
use fs_tasks_core::TaskRepo;
use serde_json::{Value, json};
use tokio::runtime::Runtime;

pub const ME: &str = "https://local.invalid/users/owner";

/// A runtime, a repository over a store, and the adapter over it, showing `Europe/Berlin` at noon on
/// Friday 2026-11-06.
pub struct World {
    pub runtime: Runtime,
    pub store: Arc<dyn Store>,
    pub repo: Arc<CalendarRepo>,
    pub tasks: Arc<TaskRepo>,
    pub adapter: Arc<CalendarAdapter>,
}

pub fn d(text: &str) -> Day {
    Day::parse(text).unwrap()
}

impl World {
    pub fn berlin() -> Self {
        Self::shown_in("Europe/Berlin")
    }

    pub fn shown_in(zone: &str) -> Self {
        let runtime = Runtime::new().unwrap();
        let store: Arc<dyn Store> =
            Arc::new(runtime.block_on(TursoStore::open_in_memory()).unwrap());
        let zone = Zone::parse(zone).unwrap();
        let tasks = Arc::new(TaskRepo::new(store.clone(), ME));
        let repo = Arc::new(
            CalendarRepo::new(store.clone(), ME)
                .with_floating_zone(zone.clone())
                .with_tasks(tasks.clone()),
        );
        let adapter = Arc::new(
            CalendarAdapter::new(repo.clone(), runtime.handle().clone(), zone)
                .frozen_at(Moment::at(d("2026-11-06"), 12, 0)),
        );
        Self {
            runtime,
            store,
            repo,
            tasks,
            adapter,
        }
    }

    /// Store `event` through the core; returns the object id as text.
    pub fn add(&self, event: Event) -> String {
        self.runtime
            .block_on(self.repo.create(event))
            .unwrap()
            .envelope
            .id
            .to_string()
    }

    /// Store JSON through the core.
    pub fn add_json(&self, value: Value) -> String {
        self.add(Event::from_value(value).unwrap())
    }

    pub fn stored(&self, key: &str) -> StoredObject {
        let id: ObjectId = key.split('#').next().unwrap().parse().unwrap();
        self.runtime
            .block_on(Calendar::get(self.repo.as_ref(), id))
            .unwrap()
            .unwrap()
    }

    /// The state machine over the real core.
    pub fn state(&self) -> CalendarState {
        let source: Arc<dyn CalendarSource> = self.adapter.clone();
        CalendarState::new(source, CalendarLabels::default(), Variant::Full)
    }

    /// What the adapter shows on the days `first` through `last`.
    pub fn items(&self, first: &str, last: &str) -> Vec<CalendarItem> {
        self.adapter.items(d(first), d(last)).unwrap()
    }

    /// The only item on the day called `title`.
    pub fn item(&self, day: &str, title: &str) -> CalendarItem {
        let found: Vec<CalendarItem> = self
            .items(day, day)
            .into_iter()
            .filter(|i| i.title == title)
            .collect();
        assert_eq!(found.len(), 1, "{title} on {day}: {found:?}");
        found.into_iter().next().unwrap()
    }
}

/// Text of a sheet field, for reading.
pub fn sheet_text(state: &CalendarState) -> String {
    match state.shown() {
        Shown::Item { sheet, .. } => {
            let mut text = format!("{} {:?} {:?}", sheet.title, sheet.marks, sheet.note);
            for section in &sheet.sections {
                for field in &section.fields {
                    text.push_str(&format!("\n{}: {}", field.label, field.value));
                }
            }
            text
        }
        other => format!("{other:?}"),
    }
}

/// An event in Berlin with something in every corner the form does not show.
pub fn rich() -> Value {
    json!({
        "@type": "Event",
        "version": "2.0",
        "uid": "urn:uuid:01a1aaf0-08a0-756c-85b2-f092789b0a01",
        "prodId": "-//Elsewhere//EN",
        "created": "2026-10-01T09:00:00Z",
        "updated": "2026-10-01T09:00:00Z",
        "sequence": 7,
        "title": "Kick-off",
        "description": "First line\nSecond line",
        "descriptionContentType": "text/plain",
        "locale": "de",
        "start": "2026-11-24T10:00:00",
        "timeZone": "Europe/Berlin",
        "endTimeZone": "Europe/London",
        "duration": "PT1H30M",
        "status": "tentative",
        "freeBusyStatus": "busy",
        "privacy": "public",
        "priority": 3,
        "keywords": {"work": true},
        "relatedTo": {"urn:uuid:other": {"relation": {"parent": true}}},
        "participants": {
            "p1": {
                "@type": "Participant",
                "name": "NLnet",
                "kind": "group",
                "roles": {"attendee": true},
                "email": "web@nlnet.example",
                "sendTo": {"imip": "mailto:web@nlnet.example"},
                "participationStatus": "needs-action",
                "expectReply": true,
                "freesynergy.net:cardUid": "urn:uuid:person"
            }
        },
        "locations": {
            "l2": {"@type": "Location", "name": "Annex", "coordinates": "geo:52.5,13.4"},
            "l1": {"@type": "Location", "name": "Main hall", "description": "Second floor",
                   "links": {"m": {"@type": "Link", "href": "https://x.example/map"}}}
        },
        "virtualLocations": {"v1": {"@type": "VirtualLocation", "uri": "https://meet.example/a"}},
        "links": {"a": {"@type": "Link", "href": "https://x.example/agenda"}},
        "alerts": {"a1": {"@type": "Alert", "trigger": {"@type": "OffsetTrigger", "offset": "-PT15M"}}},
        "freesynergy.net:source": "https://x.example/1",
        "example.org:anything": {"deep": [1, 2, {"three": true}]}
    })
}

/// A weekly series with an override and an exclusion, and corners of its own.
pub fn rich_series() -> Value {
    let mut event = rich();
    event["uid"] = json!("urn:uuid:01a1aaf0-08a0-756c-85b2-f092789b0a02");
    event["title"] = json!("Jour fixe");
    event["start"] = json!("2026-11-03T10:00:00");
    event["duration"] = json!("PT1H");
    event["recurrenceRule"] = json!({"@type": "RecurrenceRule", "frequency": "weekly", "count": 8});
    event["recurrenceOverrides"] = json!({
        "2026-11-10T10:00:00": {"title": "Moved jour fixe", "start": "2026-11-11T15:00:00"},
        "2026-11-17T10:00:00": {"excluded": true}
    });
    event.as_object_mut().unwrap().remove("endTimeZone");
    event
}

/// The event as stored, without the time of the last change.
pub fn json_of(world: &World, key: &str) -> Value {
    let mut value = world.stored(key).content;
    value.as_object_mut().unwrap().remove("updated");
    value
}

/// A folder for the files of one test, removed when the test ends (also when it fails).
pub struct Scratch(PathBuf);

impl Scratch {
    pub fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("fs-calendar-app-{}", ObjectId::new()));
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    pub fn file(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
