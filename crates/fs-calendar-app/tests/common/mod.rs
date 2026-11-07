//! Shared helpers of the tests: the real calendar server on a real Unix socket over a real store (on a
//! thread of its own), the real client, and the real adapter over it. Only the tasks are a fixed list
//! (`tasks.rs`): the wire of the tasks is proven against the real tasks server in `fs-calendar-client`.
//!
//! Pattern: Facade over the set-up.
#![allow(dead_code)] // each test file uses a different subset

use std::path::PathBuf;
use std::sync::Arc;

use fs_calendar_app::CalendarAdapter;
use fs_calendar_client::RpcCalendar;
use fs_calendar_core::{Calendar, CalendarRepo, Event, Zone};
use fs_components_workbenches::calendar::{
    CalendarItem, CalendarLabels, CalendarSource, CalendarState, Day, Moment, Shown, Variant,
};
use fs_store::{ObjectId, Store, StoredObject, TursoStore};
use serde_json::Value;
use tokio::runtime::Runtime;

mod rich;
mod scratch;
mod served;
mod tasks;
#[allow(unused_imports)] // each test file uses a different subset
pub use rich::{rich, rich_series};
#[allow(unused_imports)]
pub use scratch::Scratch;
pub use served::Served;
pub use tasks::FixedTasks;

pub const ME: &str = "https://local.invalid/users/owner";

/// A runtime, the server and its store, the client, and the adapter over the client, showing
/// `Europe/Berlin` at noon on Friday 2026-11-06. Dropped in the order of the fields: the adapter and the
/// client go before the server.
pub struct World {
    pub adapter: Arc<CalendarAdapter>,
    /// The client: what the app talks to.
    pub client: Arc<RpcCalendar>,
    pub tasks: Arc<FixedTasks>,
    /// Where the server listens, for the binary.
    pub socket: PathBuf,
    pub store: Arc<dyn Store>,
    /// The provider behind the server, for set-up and for what only old data can show (never what is
    /// under test).
    pub repo: Arc<CalendarRepo>,
    _served: Served,
    pub runtime: Runtime,
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
        let repo = Arc::new(CalendarRepo::new(store.clone(), ME).with_floating_zone(zone.clone()));
        let (served, socket) = Served::start(&repo);
        let client = Arc::new(RpcCalendar::connect(&socket).unwrap());
        let tasks = Arc::new(FixedTasks::default());
        let adapter = Arc::new(
            CalendarAdapter::new(
                client.clone(),
                client.clone(),
                runtime.handle().clone(),
                zone,
            )
            .with_tasks(tasks.clone())
            .frozen_at(Moment::at(d("2026-11-06"), 12, 0)),
        );
        Self {
            adapter,
            client,
            tasks,
            socket,
            store,
            repo,
            _served: served,
            runtime,
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

    /// The state machine through the real server and client.
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
