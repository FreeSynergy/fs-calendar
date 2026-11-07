//! `Calendar` implemented by `CalendarRepo`.
//!
//! Pattern: Facade — each trait method forwards to the inherent method of the same name (inherent
//! methods keep working for callers that do not import the trait).

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use fs_interfaces::Capabilities;
use fs_store::{ObjectId, StoredObject};

use super::handle::CalendarRepo;
use super::page::EventPage;
use super::types::{Agenda, ImportReport, Instance, Window};
use crate::calendar::Calendar;
use crate::error::Result;
use crate::feed::{CalendarFeed, CalendarListener, Subscription};
use crate::format::{Exporter, Importer};
use crate::invite::{InvitationEntry, Response};
use crate::model::Event;
use crate::revision::Revision;

#[async_trait]
impl Calendar for CalendarRepo {
    fn capabilities(&self) -> Capabilities {
        capabilities()
    }
    async fn create(&self, event: Event) -> Result<StoredObject> {
        CalendarRepo::create(self, event).await
    }
    async fn update_checked(
        &self,
        id: ObjectId,
        expected: &Revision,
        new: Event,
    ) -> Result<StoredObject> {
        CalendarRepo::update_checked(self, id, expected, new).await
    }
    async fn window(&self, from: DateTime<Utc>, to: DateTime<Utc>) -> Result<Window> {
        CalendarRepo::window(self, from, to).await
    }
    async fn get(&self, id: ObjectId) -> Result<Option<StoredObject>> {
        CalendarRepo::get(self, id).await
    }
    async fn get_event(&self, id: ObjectId) -> Result<Option<Event>> {
        CalendarRepo::get_event(self, id).await
    }
    async fn get_by_uid(&self, uid: &str) -> Result<Option<StoredObject>> {
        CalendarRepo::get_by_uid(self, uid).await
    }
    async fn update(&self, id: ObjectId, new: Event) -> Result<StoredObject> {
        CalendarRepo::update(self, id, new).await
    }
    async fn delete(&self, id: ObjectId) -> Result<()> {
        CalendarRepo::delete(self, id).await
    }
    async fn list_events(&self) -> Result<Vec<StoredObject>> {
        CalendarRepo::list_events(self).await
    }
    async fn list_page(&self, after: Option<ObjectId>, size: usize) -> Result<EventPage> {
        CalendarRepo::list_page(self, after, size).await
    }
    async fn occurrences(&self, from: DateTime<Utc>, to: DateTime<Utc>) -> Result<Vec<Instance>> {
        CalendarRepo::occurrences(self, from, to).await
    }
    async fn upcoming(&self, now: DateTime<Utc>, days: i64) -> Result<Vec<Instance>> {
        CalendarRepo::upcoming(self, now, days).await
    }
    async fn agenda(&self, from: DateTime<Utc>, to: DateTime<Utc>) -> Result<Agenda> {
        CalendarRepo::agenda(self, from, to).await
    }
    async fn migrate_time_fields(&self) -> Result<usize> {
        CalendarRepo::migrate_time_fields(self).await
    }
    async fn invite(&self, id: ObjectId, invitees: &[(&str, &str)]) -> Result<StoredObject> {
        CalendarRepo::invite(self, id, invitees).await
    }
    async fn respond(
        &self,
        id: ObjectId,
        who: &str,
        response: Response,
        comment: Option<&str>,
    ) -> Result<StoredObject> {
        CalendarRepo::respond(self, id, who, response, comment).await
    }
    async fn invitation_log(&self, id: ObjectId) -> Result<Vec<InvitationEntry>> {
        CalendarRepo::invitation_log(self, id).await
    }
    async fn import(&self, importer: &dyn Importer, text: &str) -> Result<ImportReport> {
        CalendarRepo::import(self, importer, text).await
    }
    async fn import_ics(&self, text: &str) -> Result<ImportReport> {
        CalendarRepo::import_ics(self, text).await
    }
    async fn export(&self, exporter: &dyn Exporter, ids: &[ObjectId]) -> Result<String> {
        CalendarRepo::export(self, exporter, ids).await
    }
}

impl CalendarFeed for CalendarRepo {
    fn subscribe(&self, listener: Arc<dyn CalendarListener>) -> Subscription {
        self.feed.add(listener)
    }
}

/// The store-backed provider offers every method and the live feed.
fn capabilities() -> Capabilities {
    let mut caps = Capabilities::new("fs-calendar");
    for id in [
        "event.create",
        "event.update",
        "event.delete",
        "event.read",
        "event.occurrences",
        "event.import",
        "event.export",
        "invite.create",
        "invite.answer",
        "invite.read",
        "event.subscribe",
    ] {
        caps.supported(id);
    }
    caps
}
