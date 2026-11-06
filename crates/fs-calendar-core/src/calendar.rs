//! The [`Calendar`] contract: what the calendar program offers to everyone else.
//!
//! Pattern: Repository (interface) — callers (the GUI, the hub, tests with fakes) depend on this
//! trait, never on the store-backed `CalendarRepo`.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use fs_store::{ObjectId, StoredObject};

use crate::error::Result;
use crate::format::{Exporter, IcsFormat, Importer, JsCalendarFormat};
use crate::invite::{InvitationEntry, Response};
use crate::model::{CalendarObject, Event, Task};
use crate::repo::{ImportReport, Instance};

/// Events, tasks, recurrence queries, invitations and import / export.
#[async_trait]
pub trait Calendar: Send + Sync {
    /// Validate and store a new event or task. Refused if its `uid` already exists.
    async fn create(&self, obj: CalendarObject) -> Result<StoredObject>;
    /// The stored object, if it exists and is an event or task.
    async fn get(&self, id: ObjectId) -> Result<Option<StoredObject>>;
    /// The typed object (event or task), if it exists.
    async fn get_object(&self, id: ObjectId) -> Result<Option<CalendarObject>>;
    /// The event, if `id` is an event.
    async fn get_event(&self, id: ObjectId) -> Result<Option<Event>>;
    /// The task, if `id` is a task.
    async fn get_task(&self, id: ObjectId) -> Result<Option<Task>>;
    /// Lookup by the object's `uid`.
    async fn get_by_uid(&self, uid: &str) -> Result<Option<StoredObject>>;
    /// Replace the object (`uid` and type cannot change; `sequence` follows the change).
    async fn update(&self, id: ObjectId, new: CalendarObject) -> Result<StoredObject>;
    /// Delete an object (undoable through the store).
    async fn delete(&self, id: ObjectId) -> Result<()>;
    /// All events, ordered by first start.
    async fn list_events(&self) -> Result<Vec<StoredObject>>;
    /// All tasks, ordered by due date.
    async fn list_tasks(&self) -> Result<Vec<StoredObject>>;
    /// Event instances overlapping `[from, to)`, series expanded.
    async fn occurrences(&self, from: DateTime<Utc>, to: DateTime<Utc>) -> Result<Vec<Instance>>;
    /// Instances starting from `now` for the next `days` days.
    async fn upcoming(&self, now: DateTime<Utc>, days: i64) -> Result<Vec<Instance>>;
    /// Tasks due in `[from, to)`.
    async fn tasks_due(&self, from: DateTime<Utc>, to: DateTime<Utc>) -> Result<Vec<StoredObject>>;
    /// Fill envelope times of objects written before fs-store 0.2; idempotent.
    async fn migrate_time_fields(&self) -> Result<usize>;
    /// Invite `(name, email)` pairs; participants and `Invite` log entries are one transaction.
    async fn invite(&self, id: ObjectId, invitees: &[(&str, &str)]) -> Result<StoredObject>;
    /// Answer an invitation as `who` (participant key or e-mail).
    async fn respond(
        &self,
        id: ObjectId,
        who: &str,
        response: Response,
        comment: Option<&str>,
    ) -> Result<StoredObject>;
    /// Invitation history of an object, oldest first.
    async fn invitation_log(&self, id: ObjectId) -> Result<Vec<InvitationEntry>>;
    /// Import the objects `importer` finds in `text`; known `uid`s are skipped.
    async fn import(&self, importer: &dyn Importer, text: &str) -> Result<ImportReport>;
    /// The objects as text of the format `exporter` writes.
    async fn export(&self, exporter: &dyn Exporter, ids: &[ObjectId]) -> Result<String>;

    /// Import JSCalendar JSON (1.0 or 2.0).
    async fn import_jscalendar(&self, json_text: &str) -> Result<ImportReport> {
        self.import(&JsCalendarFormat, json_text).await
    }
    /// Import an `.ics` text.
    async fn import_ics(&self, text: &str) -> Result<ImportReport> {
        self.import(&IcsFormat, text).await
    }
    /// Export objects as iCalendar text.
    async fn export_ics(&self, ids: &[ObjectId]) -> Result<String> {
        self.export(&IcsFormat, ids).await
    }
}
