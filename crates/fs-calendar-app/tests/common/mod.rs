//! Shared by the series tests: a calendar that races, a world with tasks, and small helpers.
//!
//! Pattern: none (tests).
#![allow(clippy::pedantic, dead_code)]

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use fs_admin_frontend::Values;
use fs_calendar_app::EventBackend;
use fs_calendar_core::{
    Calendar, CalendarRepo, Capabilities, Event, Instance, Result, Revision, Window,
};
use fs_calendar_tasks::LocalTasks;
use fs_objects::{ObjectId, Store, StoredObject, TursoStore};
use fs_tasks_core::TaskRepo;

pub const ICS: &str = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//x//y//EN\r\nBEGIN:VEVENT\r\nUID:ev-1\r\nDTSTAMP:20261101T000000Z\r\nDTSTART;TZID=Europe/Berlin:20261124T100000\r\nDTEND;TZID=Europe/Berlin:20261124T113000\r\nSUMMARY:Jour fixe\r\nRRULE:FREQ=WEEKLY;COUNT=4;BYDAY=TU\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
pub const ME: &str = "https://local.invalid/users/owner";

/// A calendar that changes the title of one event in the middle of a `window` read, as another writer could.
pub struct Racing {
    pub inner: Arc<CalendarRepo>,
    pub retitle: Mutex<Option<ObjectId>>,
}

#[async_trait]
impl Calendar for Racing {
    fn capabilities(&self) -> Capabilities {
        Calendar::capabilities(&*self.inner)
    }
    async fn get(&self, id: ObjectId) -> Result<Option<StoredObject>> {
        Calendar::get(&*self.inner, id).await
    }
    async fn get_event(&self, id: ObjectId) -> Result<Option<Event>> {
        Calendar::get_event(&*self.inner, id).await
    }
    async fn get_by_uid(&self, uid: &str) -> Result<Option<StoredObject>> {
        Calendar::get_by_uid(&*self.inner, uid).await
    }
    async fn list_events(&self) -> Result<Vec<StoredObject>> {
        Calendar::list_events(&*self.inner).await
    }
    async fn occurrences(&self, from: DateTime<Utc>, to: DateTime<Utc>) -> Result<Vec<Instance>> {
        Calendar::occurrences(&*self.inner, from, to).await
    }
    async fn window(&self, from: DateTime<Utc>, to: DateTime<Utc>) -> Result<Window> {
        let racing = self.retitle.lock().unwrap().take();
        if let Some(id) = racing {
            let mut e = self.inner.get_event(id).await?.unwrap();
            e.title = Some("Between the reads".into());
            self.inner.update(id, e).await?;
        }
        Calendar::window(&*self.inner, from, to).await
    }
    async fn update_checked(
        &self,
        id: ObjectId,
        expected: &Revision,
        new: Event,
    ) -> Result<StoredObject> {
        Calendar::update_checked(&*self.inner, id, expected, new).await
    }
    async fn create(&self, event: Event) -> Result<StoredObject> {
        Calendar::create(&*self.inner, event).await
    }
    async fn delete(&self, id: ObjectId) -> Result<()> {
        Calendar::delete(&*self.inner, id).await
    }
    async fn agenda(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<fs_calendar_core::Agenda> {
        Calendar::agenda(&*self.inner, from, to).await
    }
    async fn invite(&self, id: ObjectId, who: &[(&str, &str)]) -> Result<StoredObject> {
        Calendar::invite(&*self.inner, id, who).await
    }
    async fn respond(
        &self,
        id: ObjectId,
        who: &str,
        r: fs_calendar_core::Response,
        c: Option<&str>,
    ) -> Result<StoredObject> {
        Calendar::respond(&*self.inner, id, who, r, c).await
    }
    async fn invitation_log(&self, id: ObjectId) -> Result<Vec<fs_calendar_core::InvitationEntry>> {
        Calendar::invitation_log(&*self.inner, id).await
    }
}

pub struct World {
    pub rt: tokio::runtime::Runtime,
    pub repo: Arc<CalendarRepo>,
    pub tasks: Arc<TaskRepo>,
    pub racing: Arc<Racing>,
    pub backend: EventBackend,
}

pub fn world() -> World {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let store: Arc<dyn Store> = Arc::new(rt.block_on(TursoStore::open_in_memory()).unwrap());
    let tasks = Arc::new(TaskRepo::new(store.clone(), ME));
    let local = Arc::new(LocalTasks::new(tasks.clone()));
    let repo = Arc::new(
        CalendarRepo::new(store, ME)
            .with_tasks(local.clone())
            .with_task_intake(local),
    );
    rt.block_on(repo.import_ics(ICS)).unwrap();
    let racing = Arc::new(Racing {
        inner: repo.clone(),
        retitle: Mutex::new(None),
    });
    let backend = EventBackend::new(racing.clone() as Arc<dyn Calendar>, rt.handle().clone());
    World {
        rt,
        repo,
        tasks,
        racing,
        backend,
    }
}

pub fn v(pairs: &[(&str, &[&str])]) -> Values {
    pairs
        .iter()
        .map(|(k, vs)| {
            (
                (*k).to_owned(),
                vs.iter().map(|s| (*s).to_owned()).collect(),
            )
        })
        .collect()
}

pub fn range() -> Values {
    v(&[
        ("from", &["2026-11-01T00:00:00Z"]),
        ("to", &["2027-01-31T00:00:00Z"]),
    ])
}

/// The id of the first event, read from the core itself: no list of the backend has run, so no revision is remembered.
pub fn series_id(w: &World) -> String {
    let stored = w.rt.block_on(w.repo.list_events()).unwrap();
    fs_calendar_app::iri(stored[0].envelope.id)
}
