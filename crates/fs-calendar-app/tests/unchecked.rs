//! A window whose check cannot be made (the list of events fails or takes too long) still shows what
//! the server answered, and says that damage and cuts could not be checked.

mod common;

use std::sync::Arc;

use async_trait::async_trait;
use common::{World, d, rich};
use fs_calendar_app::CalendarAdapter;
use fs_calendar_core::{Calendar, Capabilities, Error, Event, Instance, Zone};
use fs_objects::{Error as StoreError, ObjectId, StoredObject};

/// The real client, except that the list of events is not to be had.
struct NoList(Arc<fs_calendar_client::RpcCalendar>);

#[async_trait]
impl Calendar for NoList {
    fn capabilities(&self) -> Capabilities {
        self.0.capabilities()
    }
    async fn get(&self, id: ObjectId) -> fs_calendar_core::Result<Option<StoredObject>> {
        self.0.get(id).await
    }
    async fn get_event(&self, id: ObjectId) -> fs_calendar_core::Result<Option<Event>> {
        self.0.get_event(id).await
    }
    async fn get_by_uid(&self, uid: &str) -> fs_calendar_core::Result<Option<StoredObject>> {
        self.0.get_by_uid(uid).await
    }
    async fn list_events(&self) -> fs_calendar_core::Result<Vec<StoredObject>> {
        Err(Error::Store(StoreError::Backend(
            "calendar server: no answer within 30 s".into(),
        )))
    }
    async fn occurrences(
        &self,
        from: chrono::DateTime<chrono::Utc>,
        to: chrono::DateTime<chrono::Utc>,
    ) -> fs_calendar_core::Result<Vec<Instance>> {
        self.0.occurrences(from, to).await
    }
}

#[test]
fn the_server_answer_stands_and_the_missing_check_is_said() {
    let w = World::berlin();
    w.add_json(rich());
    let adapter = CalendarAdapter::new(
        Arc::new(NoList(w.client.clone())),
        w.client.clone(),
        w.runtime.handle().clone(),
        Zone::parse("Europe/Berlin").unwrap(),
    );
    let read = fs_components_workbenches::calendar::CalendarSource::items(
        &adapter,
        d("2026-11-24"),
        d("2026-11-24"),
    )
    .expect("no error: the occurrences came");
    assert_eq!(read.items.len(), 1, "{:?}", read.problems);
    assert_eq!(read.problems.len(), 1, "{:?}", read.problems);
    let said = &read.problems[0].text;
    assert!(
        said.contains("could not be checked") && said.contains("30 s"),
        "{said}"
    );
}
