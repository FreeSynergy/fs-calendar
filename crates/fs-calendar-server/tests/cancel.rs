//! A client that goes away cannot leave a write half done.

mod common;

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use common::{ME, items_of, with_provider};
use fs_calendar_core::CalendarRepo;
use fs_interfaces::events_capnp::items;
use fs_store::traits::{ActivityLog, ObjectQuery, ObjectRepository};
use fs_store::{
    Activity, ActivityFilter, ActivityKind, LogEntry, ObjectId, Query, Result, Store, StoredObject,
    TursoStore,
};
use tokio::sync::{Notify, Semaphore};

const TWO: &str = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:x\r\nBEGIN:VEVENT\r\nUID:one\r\nDTSTAMP:20261101T000000Z\r\nDTSTART:20261201T090000Z\r\nSUMMARY:1\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nUID:two\r\nDTSTAMP:20261101T000000Z\r\nDTSTART:20261202T090000Z\r\nSUMMARY:2\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";

/// A store whose writes stop at a gate until the test opens it.
struct Gated {
    store: Arc<dyn Store>,
    reached: Notify,
    gate: Semaphore,
}

#[async_trait]
impl ObjectRepository for Gated {
    async fn put(&self, object: StoredObject, actor: &str) -> Result<Activity> {
        self.reached.notify_one();
        drop(self.gate.acquire().await.unwrap());
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
impl ObjectQuery for Gated {
    async fn query(&self, query: &Query) -> Result<Vec<StoredObject>> {
        self.store.query(query).await
    }

    async fn count(&self, query: &Query) -> Result<usize> {
        self.store.count(query).await
    }
}

#[async_trait]
impl ActivityLog for Gated {
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

async fn stored(items: &items::Client) -> u32 {
    let list = items.list_request().send().promise.await.unwrap();
    list.get()
        .unwrap()
        .get_page()
        .unwrap()
        .get_items()
        .unwrap()
        .len()
}

#[tokio::test(flavor = "current_thread")]
async fn an_import_dropped_while_it_waits_at_a_step_still_finishes() {
    let gated = Arc::new(Gated {
        store: Arc::new(TursoStore::open_in_memory().await.unwrap()),
        reached: Notify::new(),
        gate: Semaphore::new(0),
    });
    let repo = Arc::new(CalendarRepo::with_backend(
        gated.clone(),
        gated.clone(),
        gated.clone(),
        ME,
    ));
    with_provider(repo.clone(), repo, |root| async move {
        let items = items_of(&root).await;
        let mut import = items.import_text_request();
        import.get().set_media_type("text/calendar");
        import.get().set_text(TWO);
        let call = import.send().promise;
        let reached = gated.reached.notified();
        tokio::time::timeout(Duration::from_secs(5), async {
            tokio::select! {
                () = reached => {}
                _ = call => panic!("the import answered before its first write"),
            }
        })
        .await
        .expect("the import reached no write");
        // The call is gone now; give the server the time to hear of it, as it stands at the gate.
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert_eq!(stored(&items).await, 0, "the gate holds");

        gated.gate.add_permits(Semaphore::MAX_PERMITS);
        for _ in 0..100 {
            if stored(&items).await == 2 {
                return;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        panic!("the write stopped halfway: {} of 2", stored(&items).await);
    })
    .await;
}
