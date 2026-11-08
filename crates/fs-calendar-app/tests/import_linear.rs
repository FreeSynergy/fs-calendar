//! The import of a file by the app costs work in proportion to the file, not to the file times the
//! events already stored. The app imports the whole text in one call of the core (which reads the known
//! `uid`s once); a loop with one call per part read them again for every part. What is counted is what
//! the repository asks of the store, with the charge for a lookup by an index field described in
//! `fs-calendar-core/tests/import_linear.rs`: n and 2n events cost 1 : 2, a call per part about 1 : 4.

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use common::{ME, Scratch};
use fs_calendar_app::import_file;
use fs_calendar_core::{Calendar, CalendarRepo};
use fs_objects::traits::{ActivityLog, ObjectQuery, ObjectRepository};
use fs_objects::{
    Activity, ActivityFilter, ActivityKind, LogEntry, ObjectId, Query, Result, Store, StoredObject,
    TursoStore,
};

/// Passes everything on and counts the work the repository asks for.
struct Charging {
    store: Arc<dyn Store>,
    /// Calls of `get`, `put` and `query`, plus the rows the queries hand back and what a lookup by an
    /// index field is charged.
    work: AtomicUsize,
    /// Queries that look for an event by an index field or an OR group.
    lookups: AtomicUsize,
}

#[async_trait]
impl ObjectRepository for Charging {
    async fn put(&self, object: StoredObject, actor: &str) -> Result<Activity> {
        self.work.fetch_add(1, Ordering::Relaxed);
        self.store.put(object, actor).await
    }
    async fn put_and_log(
        &self,
        object: StoredObject,
        actor: &str,
        extra: Vec<LogEntry>,
    ) -> Result<Vec<Activity>> {
        self.work.fetch_add(1, Ordering::Relaxed);
        self.store.put_and_log(object, actor, extra).await
    }
    async fn get(&self, id: ObjectId) -> Result<Option<StoredObject>> {
        self.work.fetch_add(1, Ordering::Relaxed);
        self.store.get(id).await
    }
    async fn delete(&self, id: ObjectId, actor: &str) -> Result<Activity> {
        self.store.delete(id, actor).await
    }
}

#[async_trait]
impl ObjectQuery for Charging {
    async fn query(&self, query: &Query) -> Result<Vec<StoredObject>> {
        let found = self.store.query(query).await?;
        let mut cost = 1 + found.len();
        if !query.index.is_empty() || !query.conds.is_empty() || !query.any.is_empty() {
            self.lookups.fetch_add(1, Ordering::Relaxed);
            cost += self.store.count(&Query::new().of_type("Event")).await?;
        }
        self.work.fetch_add(cost, Ordering::Relaxed);
        Ok(found)
    }
    async fn count(&self, query: &Query) -> Result<usize> {
        self.store.count(query).await
    }
}

#[async_trait]
impl ActivityLog for Charging {
    async fn log(
        &self,
        kind: ActivityKind,
        object: ObjectId,
        actor: &str,
        payload: serde_json::Value,
    ) -> Result<Activity> {
        self.store.log(kind, object, actor, payload).await
    }
    async fn activities(&self, filter: &ActivityFilter) -> Result<Vec<Activity>> {
        self.store.activities(filter).await
    }
}

fn ics(first: usize, n: usize) -> String {
    let mut s = String::from("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//x//y//EN\r\n");
    for i in first..first + n {
        s += &format!(
            "BEGIN:VEVENT\r\nUID:ev-{i:05}\r\nDTSTAMP:20261101T000000Z\r\nDTSTART:20261130T090000Z\r\nDURATION:PT1H\r\nSUMMARY:E{i}\r\nEND:VEVENT\r\n"
        );
    }
    s + "END:VCALENDAR\r\n"
}

/// Work to import a file of `n` new events with the app, into a store that holds `before` others.
fn work_for(before: usize, n: usize) -> usize {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let dir = Scratch::new();
    let charging = Arc::new(Charging {
        store: Arc::new(runtime.block_on(TursoStore::open_in_memory()).unwrap()),
        work: AtomicUsize::new(0),
        lookups: AtomicUsize::new(0),
    });
    let repo = CalendarRepo::with_backend(charging.clone(), charging.clone(), charging.clone(), ME);
    let (first, second) = (dir.file("before.ics"), dir.file("file.ics"));
    std::fs::write(&first, ics(10_000, before)).unwrap();
    std::fs::write(&second, ics(0, n)).unwrap();
    let calendar: &dyn Calendar = &repo;
    let made = import_file(calendar, runtime.handle(), &first).unwrap();
    assert_eq!(made.created, before);
    charging.work.store(0, Ordering::Relaxed);
    let summary = import_file(calendar, runtime.handle(), &second).unwrap();
    assert_eq!(
        (summary.created, summary.failed.len()),
        (n, 0),
        "{}",
        summary.message()
    );
    charging.work.load(Ordering::Relaxed)
}

#[test]
fn importing_a_file_twice_as_long_costs_twice_the_work() {
    for before in [0, 40] {
        let (small, large) = (work_for(before, 60), work_for(before, 120));
        #[allow(clippy::cast_precision_loss)]
        let ratio = large as f64 / small as f64;
        eprintln!("{before} stored: 60 events {small}, 120 events {large}, ratio {ratio:.2}");
        assert!(
            ratio < 2.5,
            "{before} stored: {small} -> {large}, ratio {ratio:.2}"
        );
    }
}
