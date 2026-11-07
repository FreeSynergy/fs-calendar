//! Writes in flight are bounded: a request that has been read and parsed waits for its turn holding an
//! event in memory, so only a few may do that at once.

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use common::{ME, create, items_of, json, with_provider};
use fs_calendar_core::{Calendar, CalendarRepo, Capabilities, Event, Instance, Result};
use fs_calendar_server::MAX_CONCURRENT_WRITES;
use fs_store::{ObjectId, Store, StoredObject, TursoStore};

/// A calendar that notes how many creates are inside it at once, and keeps each a moment.
struct Watching {
    repo: Arc<CalendarRepo>,
    inside: AtomicUsize,
    most: AtomicUsize,
}

#[async_trait]
impl Calendar for Watching {
    fn capabilities(&self) -> Capabilities {
        self.repo.capabilities()
    }
    async fn get(&self, id: ObjectId) -> Result<Option<StoredObject>> {
        Calendar::get(&*self.repo, id).await
    }
    async fn get_event(&self, id: ObjectId) -> Result<Option<Event>> {
        Calendar::get_event(&*self.repo, id).await
    }
    async fn get_by_uid(&self, uid: &str) -> Result<Option<StoredObject>> {
        Calendar::get_by_uid(&*self.repo, uid).await
    }
    async fn list_events(&self) -> Result<Vec<StoredObject>> {
        Calendar::list_events(&*self.repo).await
    }
    async fn occurrences(
        &self,
        from: chrono::DateTime<chrono::Utc>,
        to: chrono::DateTime<chrono::Utc>,
    ) -> Result<Vec<Instance>> {
        Calendar::occurrences(&*self.repo, from, to).await
    }
    async fn create(&self, event: Event) -> Result<StoredObject> {
        let now = self.inside.fetch_add(1, Ordering::SeqCst) + 1;
        self.most.fetch_max(now, Ordering::SeqCst);
        tokio::time::sleep(Duration::from_millis(30)).await;
        let made = Calendar::create(&*self.repo, event).await;
        self.inside.fetch_sub(1, Ordering::SeqCst);
        made
    }
}

#[tokio::test(flavor = "current_thread")]
async fn only_a_few_writes_are_in_flight_and_all_of_them_are_done() {
    let store: Arc<dyn Store> = Arc::new(TursoStore::open_in_memory().await.unwrap());
    let repo = Arc::new(CalendarRepo::new(store, ME));
    let watching = Arc::new(Watching {
        repo: repo.clone(),
        inside: AtomicUsize::new(0),
        most: AtomicUsize::new(0),
    });
    with_provider(watching.clone(), repo, |root| async move {
        let items = items_of(&root).await;
        let calls: Vec<_> = (0..16)
            .map(|i| {
                let mut req = items.create_request();
                req.get()
                    .set_jscalendar(json(&format!("c{i}"), "C", "2026-12-01T10:00:00").as_str());
                req.send().promise
            })
            .collect();
        for call in calls {
            call.await.expect("every write is done in the end");
        }
        create(&items, "after", "After", "2026-12-02T10:00:00").await;
    })
    .await;
    let most = watching.most.load(Ordering::SeqCst);
    assert!(most >= 2, "the writes did run side by side: {most}");
    assert!(
        most <= MAX_CONCURRENT_WRITES,
        "{most} writes at once, at most {MAX_CONCURRENT_WRITES}"
    );
}
