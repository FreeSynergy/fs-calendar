//! Handing the tasks of an import to the task program.
//!
//! Pattern: Strategy (interface segregation) — what an import needs, apart from what the view needs:
//! hand over a `.ics` text, or one task found in a JSCalendar file, and say what became of it.

use async_trait::async_trait;
use fs_objects::ObjectId;

use super::error::TasksError;
use crate::repo::ImportReport;

/// `@type` of a JSCalendar object that belongs to the task program.
pub const TASK_TYPE: &str = "Task";

/// What became of one task handed over.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Forwarded {
    /// Stored, with this id.
    Created(ObjectId),
    /// A task with this `uid` exists already (also when it is found only while storing: a conflict);
    /// it is not overwritten.
    Skipped,
    /// The task program does not accept this task (invalid, too large); the text says why. Only this
    /// task is lost, the import goes on.
    Refused(String),
}

/// The task program, as far as an import needs it.
#[async_trait]
pub trait TaskIntake: Send + Sync {
    /// Hand over the `VTODO`s of an `.ics` text; the report is the task program's own.
    async fn import_ics(&self, text: &str) -> Result<ImportReport, TasksError>;

    /// Hand over one task as JSCalendar JSON. `uid` is its `uid` as written (the task program matches it as it keeps it): a known one is
    /// [`Forwarded::Skipped`] and never overwritten.
    async fn forward(&self, uid: &str, jscalendar: &str) -> Result<Forwarded, TasksError>;
}
