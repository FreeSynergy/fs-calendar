//! Writes in flight are bounded, also when the clients go away: a request that has been read and parsed
//! waits for its turn holding an event in memory, so only a few may do that at once, and the turn belongs
//! to the work, not to the call (the work goes on when the call is dropped).

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use common::{ME, create, items_of, json, with_provider};
use fs_calendar_core::{Calendar, CalendarRepo, Capabilities, Event, Instance, Result};
use fs_calendar_server::MAX_CONCURRENT_WRITES;
use fs_store::{ObjectId, Store, StoredObject, TursoStore};

/// A calendar that counts how many creates are inside it at once; each stays until the test opens the gate.
struct Watching {
    repo: Arc<CalendarRepo>,
    inside: AtomicUsize,
    most: AtomicUsize,
    gate: tokio::sync::Semaphore,
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
        drop(self.gate.acquire().await.unwrap());
        let made = Calendar::create(&*self.repo, event).await;
        self.inside.fetch_sub(1, Ordering::SeqCst);
        made
    }
}

#[tokio::test(flavor = "current_thread")]
async fn clients_that_go_away_do_not_let_more_writes_run_than_the_limit() {
    let store: Arc<dyn Store> = Arc::new(TursoStore::open_in_memory().await.unwrap());
    let repo = Arc::new(CalendarRepo::new(store, ME));
    let watching = Arc::new(Watching {
        repo: repo.clone(),
        inside: AtomicUsize::new(0),
        most: AtomicUsize::new(0),
        gate: tokio::sync::Semaphore::new(0),
    });
    let seen = watching.clone();
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
        // wait until the limit is reached inside, then every client goes away
        for _ in 0..200 {
            if seen.inside.load(Ordering::SeqCst) == MAX_CONCURRENT_WRITES {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert_eq!(seen.inside.load(Ordering::SeqCst), MAX_CONCURRENT_WRITES);
        drop(calls);
        // the server hears of it; whatever it would let in now, it would let in by now
        tokio::time::sleep(Duration::from_millis(300)).await;
        seen.gate.add_permits(1000);
        for _ in 0..200 {
            if seen.inside.load(Ordering::SeqCst) == 0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        create(&items, "after", "After", "2026-12-02T10:00:00").await;
    })
    .await;
    let most = watching.most.load(Ordering::SeqCst);
    assert_eq!(
        most, MAX_CONCURRENT_WRITES,
        "{most} writes at once, the limit is {MAX_CONCURRENT_WRITES}"
    );
}
