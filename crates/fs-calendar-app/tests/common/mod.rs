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
use serde_json::Value;
use tokio::runtime::Runtime;

mod rich;
#[allow(unused_imports)] // each test file uses a different subset
pub use rich::{rich, rich_series};

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
                .with_tasks(tasks.clone())
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

    /// Store JSON the way an older version could have: the core takes it without its overrides (it
    /// refuses a broken one now), which are put into the store behind its back.
    pub fn add_json_past_core(&self, mut value: Value) -> String {
        let overrides = value
            .as_object_mut()
            .and_then(|members| members.remove("recurrenceOverrides"));
        let key = self.add_json(value);
        if let Some(overrides) = overrides {
            self.put_overrides(&key, overrides);
        }
        key
    }

    /// Put `overrides` into the stored event behind the core's back.
    pub fn put_overrides(&self, key: &str, overrides: Value) {
        let mut object = self.stored(key);
        object.content["recurrenceOverrides"] = overrides;
        self.runtime.block_on(self.store.put(object, ME)).unwrap();
    }

    pub fn stored(&self, key: &str) -> StoredObject {
        let id: ObjectId = key.split('#').next().unwrap().parse().unwrap();
        self.runtime
            .block_on(Calendar::get(self.repo.as_ref(), id))
            .unwrap()
            .unwrap()
    }

    /// The content of the event with this `uid`.
    pub fn stored_uid(&self, uid: &str) -> Value {
        self.runtime
            .block_on(Calendar::get_by_uid(self.repo.as_ref(), uid))
            .unwrap()
            .unwrap()
            .content
    }

    /// The state machine over the real core.
    pub fn state(&self) -> CalendarState {
        let source: Arc<dyn CalendarSource> = self.adapter.clone();
        CalendarState::new(source, CalendarLabels::default(), Variant::Full)
    }

    /// What the adapter shows on the days `first` through `last`.
    pub fn items(&self, first: &str, last: &str) -> Vec<CalendarItem> {
        self.adapter.items(d(first), d(last)).unwrap().items
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
