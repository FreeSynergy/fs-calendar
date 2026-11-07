//! The [`Calendar`] contract: what the calendar program offers to everyone else.
//!
//! Pattern: Repository (interface) + Null Object — callers (the GUI, the hub, tests with fakes)
//! depend on this trait, never on the store-backed `CalendarRepo`. It is the Rust form of
//! `Events`, `Items` and `Invitations` in `events.capnp` (fs-interfaces). Like a parent class
//! (ADR 0045) every provider has every method: one it does not override answers an explicit
//! [`Unsupported`], and `capabilities` reports the same.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use fs_interfaces::{Capabilities, Unsupported};
use fs_store::{ObjectId, StoredObject};

use crate::error::Result;
use crate::format::{Exporter, Importer, TextFormat};
use crate::invite::{InvitationEntry, Response};
use crate::model::Event;
use crate::repo::{Agenda, EventPage, ImportReport, Instance, Window};
use crate::revision::Revision;

/// The explicit "not possible" of a method a provider does not override.
pub(crate) fn not_possible<T>(capability: &str) -> Result<T> {
    Err(Unsupported::new(capability, "this provider does not offer it").into())
}

/// Events, recurrence queries, the agenda (events with due tasks), invitations and import / export.
///
/// Interface status: draft (ADR 0045).
#[async_trait]
pub trait Calendar: Send + Sync {
    /// What this provider can do, per capability id.
    fn capabilities(&self) -> Capabilities;
    /// The stored object, if it exists and is an event.
    async fn get(&self, id: ObjectId) -> Result<Option<StoredObject>>;
    /// The event, if `id` is an event.
    async fn get_event(&self, id: ObjectId) -> Result<Option<Event>>;
    /// Lookup by the object's `uid`.
    async fn get_by_uid(&self, uid: &str) -> Result<Option<StoredObject>>;
    /// All events, ordered by first start.
    async fn list_events(&self) -> Result<Vec<StoredObject>>;
    /// At most `size` events after the event `after` (`None` = from the start), in the order of
    /// their ids. A provider with a store cuts the page there; the default cuts it from
    /// [`Calendar::list_events`], which reads every event.
    async fn list_page(&self, after: Option<ObjectId>, size: usize) -> Result<EventPage> {
        Ok(EventPage::of_all(self.list_events().await?, after, size))
    }
    /// Event instances overlapping `[from, to)`, series expanded.
    async fn occurrences(&self, from: DateTime<Utc>, to: DateTime<Utc>) -> Result<Vec<Instance>>;

    /// `event.create`: validate and store a new event. Refused if its `uid` already exists.
    async fn create(&self, _event: Event) -> Result<StoredObject> {
        not_possible("event.create")
    }
    /// `event.update`: replace the event (`uid` cannot change; `sequence` follows the change).
    async fn update(&self, _id: ObjectId, _new: Event) -> Result<StoredObject> {
        not_possible("event.update")
    }
    /// `event.update`, compare-and-set: replace the event only if it is still at `expected` (the
    /// [`Revision`] the caller read); otherwise [`Error::Changed`](crate::Error::Changed) and
    /// nothing is written. The check and the write are one step.
    async fn update_checked(
        &self,
        _id: ObjectId,
        _expected: &Revision,
        _new: Event,
    ) -> Result<StoredObject> {
        not_possible("event.update")
    }
    /// `event.delete`: delete an object (undoable through the store).
    async fn delete(&self, _id: ObjectId) -> Result<()> {
        not_possible("event.delete")
    }
    /// `event.occurrences` with what could not be answered: like [`Calendar::occurrences`], but a
    /// damaged event or override is named in the result instead of costing the whole answer. The
    /// default is the provider's `occurrences` and no damage to report: a provider that knows no
    /// damage has none to name.
    async fn window(&self, from: DateTime<Utc>, to: DateTime<Utc>) -> Result<Window> {
        Ok(Window {
            instances: self.occurrences(from, to).await?,
            damaged: vec![],
        })
    }
    /// Instances starting from `now` for the next `days` days.
    async fn upcoming(&self, now: DateTime<Utc>, days: i64) -> Result<Vec<Instance>> {
        self.occurrences(now, crate::limits::after_days(now, days))
            .await
    }
    /// Event instances in `[from, to)` together with the tasks due then, read through the
    /// `fs-tasks` interface (empty without an attached task source).
    async fn agenda(&self, _from: DateTime<Utc>, _to: DateTime<Utc>) -> Result<Agenda> {
        not_possible("event.occurrences")
    }
    /// Fill envelope times of objects written before fs-store 0.2; idempotent.
    async fn migrate_time_fields(&self) -> Result<usize> {
        not_possible("event.update")
    }
    /// `invite.create`: invite `(name, email)` pairs; participants and `Invite` log entries are one transaction.
    async fn invite(&self, _id: ObjectId, _invitees: &[(&str, &str)]) -> Result<StoredObject> {
        not_possible("invite.create")
    }
    /// `invite.answer`: answer an invitation as `who` (participant key or e-mail).
    async fn respond(
        &self,
        _id: ObjectId,
        _who: &str,
        _response: Response,
        _comment: Option<&str>,
    ) -> Result<StoredObject> {
        not_possible("invite.answer")
    }
    /// `invite.read`: invitation history of an object, oldest first.
    async fn invitation_log(&self, _id: ObjectId) -> Result<Vec<InvitationEntry>> {
        not_possible("invite.read")
    }
    /// `event.import`: import the `VEVENT`s of an `.ics` text (`VTODO`s go to the attached task
    /// source); known `uid`s are skipped.
    async fn import_ics(&self, _text: &str) -> Result<ImportReport> {
        not_possible("event.import")
    }
    /// `event.import`: import the objects `importer` finds in `text`; known `uid`s are skipped.
    async fn import(&self, _importer: &dyn Importer, _text: &str) -> Result<ImportReport> {
        not_possible("event.import")
    }
    /// `event.export`: the objects as text of the format `exporter` writes.
    async fn export(&self, _exporter: &dyn Exporter, _ids: &[ObjectId]) -> Result<String> {
        not_possible("event.export")
    }

    /// `importText` of the contract: import `text` of `media_type` (`text/calendar` or
    /// `application/jscalendar+json`). Any other media type is an [`Error::MediaType`](crate::Error::MediaType).
    async fn import_text(&self, media_type: &str, text: &str) -> Result<ImportReport> {
        match TextFormat::from_media_type(media_type)? {
            TextFormat::Ics => self.import_ics(text).await,
            format @ TextFormat::JsCalendar => self.import(format.importer(), text).await,
        }
    }
    /// `exportText` of the contract: the events as text of `media_type`, same types as
    /// [`Calendar::import_text`].
    async fn export_text(&self, ids: &[ObjectId], media_type: &str) -> Result<String> {
        let format = TextFormat::from_media_type(media_type)?;
        self.export(format.exporter(), ids).await
    }
    /// Import JSCalendar JSON (1.0 or 2.0).
    async fn import_jscalendar(&self, json_text: &str) -> Result<ImportReport> {
        self.import_text("application/jscalendar+json", json_text)
            .await
    }
    /// Export events as iCalendar text.
    async fn export_ics(&self, ids: &[ObjectId]) -> Result<String> {
        self.export_text(ids, "text/calendar").await
    }
}
