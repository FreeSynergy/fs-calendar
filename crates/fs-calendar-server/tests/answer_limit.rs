//! The answer of a provider that reports no cut and still holds more than a message carries is
//! refused as too large; the connection lives. (The calendar itself cuts before that, see `cut.rs`.)

mod common;

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use common::{ME, ask_window, create_value, items_of, with_provider};
use fs_calendar_core::{Calendar, CalendarFeed, CalendarRepo, Capabilities, Instance, Result};
use fs_interfaces::rpc::{Problem, TOO_LARGE};
use fs_objects::{ObjectId, Store, StoredObject, TursoStore};

/// Answers every window with each instance four times and names no damage and no cut.
struct Fourfold(Arc<CalendarRepo>);

#[async_trait]
impl Calendar for Fourfold {
    fn capabilities(&self) -> Capabilities {
        Capabilities::new("fourfold")
    }
    async fn get(&self, id: ObjectId) -> Result<Option<StoredObject>> {
        self.0.get(id).await
    }
    async fn get_event(&self, id: ObjectId) -> Result<Option<fs_calendar_core::Event>> {
        self.0.get_event(id).await
    }
    async fn get_by_uid(&self, uid: &str) -> Result<Option<StoredObject>> {
        self.0.get_by_uid(uid).await
    }
    async fn list_events(&self) -> Result<Vec<StoredObject>> {
        self.0.list_events().await
    }
    async fn create(&self, event: fs_calendar_core::Event) -> Result<StoredObject> {
        self.0.create(event).await
    }
    async fn occurrences(&self, from: DateTime<Utc>, to: DateTime<Utc>) -> Result<Vec<Instance>> {
        let one = self.0.occurrences(from, to).await?;
        let n = one.len() * 4;
        Ok(one.into_iter().cycle().take(n).collect())
    }
}

#[tokio::test(flavor = "current_thread")]
async fn a_sum_of_good_events_above_the_answer_limit_is_too_large_and_the_connection_lives() {
    let store: Arc<dyn Store> = Arc::new(TursoStore::open_in_memory().await.unwrap());
    let repo = Arc::new(CalendarRepo::new(store, ME));
    let feed: Arc<dyn CalendarFeed> = repo.clone();
    with_provider(Arc::new(Fourfold(repo)), feed, |root| async move {
        let items = items_of(&root).await;
        let series = serde_json::json!({"@type":"Event","version":"2.0","uid":"wide","title":"Wide",
            "start":"2026-12-01T10:00:00","timeZone":"Etc/UTC","duration":"PT1H",
            "description":"x".repeat(1024 * 1024),
            "recurrenceRule":{"@type":"RecurrenceRule","frequency":"daily"}});
        create_value(&items, &series).await;
        let few = ask_window(&items, "2026-12-01T00:00:00Z", "2026-12-03T00:00:00Z")
            .await
            .expect("eight megabytes fit");
        assert_eq!(few.objects.len(), 8);
        let error = ask_window(&items, "2026-12-01T00:00:00Z", "2026-12-21T00:00:00Z")
            .await
            .err()
            .expect("eighty megabytes do not");
        let p = Problem::from_error(&error).unwrap();
        assert_eq!((p.problem_type.as_str(), p.status), (TOO_LARGE, 413));
        assert!(p.ext_u64("size").unwrap() > p.ext_u64("limit").unwrap());
        assert!(
            ask_window(&items, "2026-12-01T00:00:00Z", "2026-12-02T00:00:00Z")
                .await
                .is_ok()
        );
    })
    .await;
}
