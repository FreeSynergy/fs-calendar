//! A provider that names thousands of damaged events does not make the answer grow without bound: the
//! list stops at `MAX_DAMAGED` entries and the last one says how many more there are.

mod common;

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use common::{ME, ask_window, items_of, with_provider};
use fs_calendar_core::{
    Calendar, CalendarFeed, CalendarRepo, Capabilities, Damaged, Instance, Result, Window,
};
use fs_calendar_server::MAX_DAMAGED;
use fs_store::{ObjectId, Store, StoredObject, TursoStore};

/// Names `n` damaged events, every one with a long reason, and answers nothing else.
struct Broken(usize);

#[async_trait]
impl Calendar for Broken {
    fn capabilities(&self) -> Capabilities {
        Capabilities::new("broken")
    }
    async fn get(&self, _: ObjectId) -> Result<Option<StoredObject>> {
        Ok(None)
    }
    async fn get_event(&self, _: ObjectId) -> Result<Option<fs_calendar_core::Event>> {
        Ok(None)
    }
    async fn get_by_uid(&self, _: &str) -> Result<Option<StoredObject>> {
        Ok(None)
    }
    async fn list_events(&self) -> Result<Vec<StoredObject>> {
        Ok(vec![])
    }
    async fn occurrences(&self, _: DateTime<Utc>, _: DateTime<Utc>) -> Result<Vec<Instance>> {
        Ok(vec![])
    }
    async fn window(&self, _: DateTime<Utc>, _: DateTime<Utc>) -> Result<Window> {
        let damaged = (0..self.0)
            .map(|i| Damaged {
                object: ObjectId::new(),
                key: None,
                reason: format!("event {i} does not read"),
                cut: false,
            })
            .collect();
        Ok(Window {
            instances: vec![],
            damaged,
        })
    }
}

async fn damaged_for(n: usize) -> Vec<String> {
    let store: Arc<dyn Store> = Arc::new(TursoStore::open_in_memory().await.unwrap());
    let repo = Arc::new(CalendarRepo::new(store, ME));
    let feed: Arc<dyn CalendarFeed> = repo;
    let told = std::rc::Rc::new(std::cell::RefCell::new(vec![]));
    let slot = told.clone();
    with_provider(Arc::new(Broken(n)), feed, |root| async move {
        let items = items_of(&root).await;
        let asked = ask_window(&items, "2026-12-01T00:00:00Z", "2026-12-02T00:00:00Z")
            .await
            .unwrap();
        *slot.borrow_mut() = asked.damaged.into_iter().map(|d| d.1).collect();
    })
    .await;
    told.take()
}

#[tokio::test(flavor = "current_thread")]
async fn the_list_of_damaged_stops_and_says_how_many_more_there_are() {
    let few = damaged_for(MAX_DAMAGED).await;
    assert_eq!(few.len(), MAX_DAMAGED, "all that fit are told");
    assert!(few.last().unwrap().ends_with("does not read"));

    let many = damaged_for(MAX_DAMAGED + 2_500).await;
    assert_eq!(many.len(), MAX_DAMAGED + 1);
    assert_eq!(
        many[MAX_DAMAGED - 1],
        format!("event {} does not read", MAX_DAMAGED - 1)
    );
    assert!(
        many[MAX_DAMAGED].contains("2500 more"),
        "{:?}",
        many[MAX_DAMAGED]
    );
}
