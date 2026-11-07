//! Reading all pages must cost a number of rows that grows with the data, not with its square. The
//! count is of rows the store returns, not of seconds: a fixed time says nothing on another machine.
//! The store is behind a counting decorator under the provider.

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use common::{ME, items_of, ms, read_pages, with_calendar_server};
use fs_calendar_core::CalendarRepo;
use fs_calendar_server::CalendarServer;
use fs_store::traits::{ActivityLog, ObjectQuery, ObjectRepository};
use fs_store::{
    Activity, ActivityFilter, ActivityKind, LogEntry, ObjectId, Query, Result, Store, StoredObject,
    TursoStore,
};

/// Passes everything on and counts the objects the queries hand back.
struct Counting {
    store: Arc<dyn Store>,
    rows: AtomicUsize,
}

#[async_trait]
impl ObjectRepository for Counting {
    async fn put(&self, object: StoredObject, actor: &str) -> Result<Activity> {
        self.store.put(object, actor).await
    }
    async fn put_and_log(
        &self,
        object: StoredObject,
        actor: &str,
        extra: Vec<LogEntry>,
    ) -> Result<Vec<Activity>> {
        self.store.put_and_log(object, actor, extra).await
    }
    async fn get(&self, id: ObjectId) -> Result<Option<StoredObject>> {
        self.store.get(id).await
    }
    async fn delete(&self, id: ObjectId, actor: &str) -> Result<Activity> {
        self.store.delete(id, actor).await
    }
}

#[async_trait]
impl ObjectQuery for Counting {
    async fn query(&self, query: &Query) -> Result<Vec<StoredObject>> {
        let found = self.store.query(query).await?;
        self.rows.fetch_add(found.len(), Ordering::Relaxed);
        Ok(found)
    }
    async fn count(&self, query: &Query) -> Result<usize> {
        self.store.count(query).await
    }
}

#[async_trait]
impl ActivityLog for Counting {
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

const PAGE: usize = 8;

/// What is read: every page of `list`, or the instances of one window with every event in it.
#[derive(Clone, Copy)]
enum Read {
    Pages,
    Window,
}

/// Rows read to read `n` events with `how`, and how many events came back.
async fn rows_for(n: usize, how: Read) -> (usize, usize) {
    let counting = Arc::new(Counting {
        store: Arc::new(TursoStore::open_in_memory().await.unwrap()),
        rows: AtomicUsize::new(0),
    });
    let repo = Arc::new(CalendarRepo::with_backend(
        counting.clone(),
        counting.clone(),
        counting.clone(),
        ME,
    ));
    let server = CalendarServer::new(repo.clone(), repo).with_page_size(PAGE);
    let seen = std::rc::Rc::new(std::cell::Cell::new(0));
    let slot = seen.clone();
    let counted = counting.clone();
    with_calendar_server(server, |root| async move {
        let items = items_of(&root).await;
        for i in 0..n {
            let start = format!("2026-12-{:02}T{:02}:00:00", 1 + i % 28, i % 24);
            let mut call = items.create_request();
            call.get()
                .set_jscalendar(common::json(&format!("u-{i:05}"), "E", &start).as_str());
            call.send().promise.await.unwrap();
        }
        counted.rows.store(0, Ordering::Relaxed);
        match how {
            Read::Pages => slot.set(read_pages(&items, |_| async {}).await.len()),
            Read::Window => {
                let mut call = items.occurrences_request();
                call.get()
                    .init_from()
                    .set_unix_millis(ms("2026-11-30T00:00:00Z"));
                call.get()
                    .init_to()
                    .set_unix_millis(ms("2027-01-05T00:00:00Z"));
                let answer = call.send().promise.await.unwrap();
                slot.set(answer.get().unwrap().get_items().unwrap().len() as usize);
            }
        }
    })
    .await;
    (counting.rows.load(Ordering::Relaxed), seen.get())
}

#[tokio::test(flavor = "current_thread")]
async fn reading_all_pages_or_a_window_costs_rows_in_proportion_to_the_data() {
    for (name, how) in [("list", Read::Pages), ("occurrences", Read::Window)] {
        let (small, seen_small) = rows_for(80, how).await;
        let (large, seen_large) = rows_for(160, how).await;
        assert_eq!(
            (seen_small, seen_large),
            (80, 160),
            "{name}: every event once"
        );
        #[allow(clippy::cast_precision_loss)]
        let ratio = large as f64 / small as f64;
        eprintln!(
            "{name}: {small} rows for 80 events, {large} rows for 160 events, ratio {ratio:.2}"
        );
        // Linear is about 2, a whole list read for every page is about 4.
        assert!(
            ratio < 3.0,
            "{name}: {small} -> {large} rows, ratio {ratio:.2}"
        );
        assert!(
            large < 2 * 160 + 10 * 160 / PAGE,
            "{name}: {large} rows for 160 events"
        );
    }
}
