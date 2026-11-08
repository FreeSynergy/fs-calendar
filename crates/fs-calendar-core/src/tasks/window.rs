//! Tasks in a time window: the one question the calendar view asks.
//!
//! Pattern: Strategy (interface segregation) — one method, so an adapter over the RPC client of
//! `tasks.capnp`, an in-process provider and a fake in a test are interchangeable behind
//! `dyn TasksInWindow`.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use fs_objects::{Envelope, ObjectId};

use super::error::TasksError;

/// A task as `tasks.capnp` hands it over: its envelope and its JSCalendar 2.0 text. The calendar
/// does not read the task; a frontend that shows it reads the JSON.
#[derive(Clone, Debug, PartialEq)]
pub struct DueTask {
    /// The id the task has in its store.
    pub id: ObjectId,
    /// Its AS2 envelope (`Task.envelope`).
    pub envelope: Envelope,
    /// The JSCalendar 2.0 `Task` (`Task.jscalendar`, parsed).
    pub jscalendar: serde_json::Value,
}

/// The tasks due in a window.
#[async_trait]
pub trait TasksInWindow: Send + Sync {
    /// The tasks due in `[from, to)`, ordered by due date, undated ones are not among them.
    async fn due_between(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<DueTask>, TasksError>;
}
