//! `Calendar` over the socket: the methods the app uses.
//!
//! Pattern: Adapter — each method is one call of `Items` (`update_checked` is `updateIf`). Every
//! method not overridden answers the explicit `Unsupported` of the provider rule (ADR 0045):
//! invitations, `agenda`, `migrate_time_fields`. `import` and `export` take a format: the media type
//! of the importer goes to the server (`importText`), and an exporter runs here on the events the
//! server sends, as a format is pure and the app brings its own (it marks what it releases).

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use fs_calendar_core::{
    Calendar, Event, Exporter, ImportReport, Importer, Instance, Result, Revision,
};
use fs_interfaces::Capabilities;
use fs_store::{ObjectId, StoredObject};

use crate::RpcCalendar;
use crate::calls::{events, lists, transfer};
use crate::read::unreadable;

const ICS: &str = "text/calendar";

fn event_of(object: &StoredObject) -> Result<Event> {
    Event::from_value(object.content.clone()).map_err(unreadable)
}

#[async_trait]
impl Calendar for RpcCalendar {
    fn capabilities(&self) -> Capabilities {
        self.report()
    }

    async fn get(&self, id: ObjectId) -> Result<Option<StoredObject>> {
        let read = self.call(move |r| async move { events::get(&r.items, id).await });
        Ok(read.await?.map(|revised| revised.object))
    }

    async fn get_event(&self, id: ObjectId) -> Result<Option<Event>> {
        self.get(id).await?.as_ref().map(event_of).transpose()
    }

    async fn get_by_uid(&self, uid: &str) -> Result<Option<StoredObject>> {
        let uid = uid.to_string();
        self.call(move |r| async move { events::get_by_uid(&r.items, uid).await })
            .await
    }

    async fn list_events(&self) -> Result<Vec<StoredObject>> {
        self.call(|r| async move { lists::list(&r.items).await })
            .await
    }

    async fn occurrences(&self, from: DateTime<Utc>, to: DateTime<Utc>) -> Result<Vec<Instance>> {
        self.call(move |r| async move { lists::occurrences(&r.items, from, to).await })
            .await
    }

    async fn create(&self, event: Event) -> Result<StoredObject> {
        let text = event.to_value().to_string();
        self.call(move |r| async move { events::create(&r.items, text).await })
            .await
    }

    async fn update(&self, id: ObjectId, new: Event) -> Result<StoredObject> {
        let text = new.to_value().to_string();
        self.call(move |r| async move { events::update(&r.items, id, text).await })
            .await
    }

    async fn update_checked(
        &self,
        id: ObjectId,
        expected: &Revision,
        new: Event,
    ) -> Result<StoredObject> {
        let (text, expected) = (new.to_value().to_string(), *expected);
        self.call(move |r| async move { events::update_if(&r.items, id, expected, text).await })
            .await
    }

    async fn delete(&self, id: ObjectId) -> Result<()> {
        self.call(move |r| async move { events::delete(&r.items, id).await })
            .await
    }

    async fn import_text(&self, media_type: &str, text: &str) -> Result<ImportReport> {
        let (media_type, text) = (media_type.to_string(), text.to_string());
        self.call(move |r| async move { transfer::import_text(&r.items, media_type, text).await })
            .await
    }

    async fn import_ics(&self, text: &str) -> Result<ImportReport> {
        self.import_text(ICS, text).await
    }

    async fn import(&self, importer: &dyn Importer, text: &str) -> Result<ImportReport> {
        self.import_text(importer.media_type(), text).await
    }

    async fn export_text(&self, ids: &[ObjectId], media_type: &str) -> Result<String> {
        let (ids, media_type) = (ids.to_vec(), media_type.to_string());
        self.call(move |r| async move { transfer::export_text(&r.items, ids, media_type).await })
            .await
    }

    async fn export(&self, exporter: &dyn Exporter, ids: &[ObjectId]) -> Result<String> {
        let mut found = Vec::with_capacity(ids.len());
        for id in ids {
            let object = self
                .get(*id)
                .await?
                .ok_or(fs_calendar_core::Error::NotFound(*id))?;
            found.push(event_of(&object)?);
        }
        exporter.export(&found)
    }
}
