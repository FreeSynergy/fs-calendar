//! `TaskLines`: the tasks due in a time, as the calendar shows them.
//!
//! Pattern: Strategy — a narrow trait with a type of its own, so the calendar program depends on
//! `dyn TaskLines` and on no code of the tasks program (ADR 0021, 0050). [`RpcTaskLines`](crate::RpcTaskLines)
//! is the implementation over `tasks.capnp`; the calendar can run without one.

use std::fmt;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use fs_store::ObjectId;
use serde_json::Value;

/// A task as the calendar needs it: the id and name from the envelope, the JSCalendar `Task` as sent
/// (title, `due`, `timeZone`, `showWithoutTime`, `progress`, `description`, marks). Read only.
#[derive(Debug, Clone, PartialEq)]
pub struct TaskLine {
    /// The id of the task in the tasks program.
    pub id: ObjectId,
    /// The name on the envelope (the title, or the placeholder of an untitled task).
    pub name: String,
    /// The JSCalendar 2.0 `Task` as JSON.
    pub content: Value,
}

/// Tasks could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TasksUnread(pub String);

impl fmt::Display for TasksUnread {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for TasksUnread {}

/// The tasks due in `[from, to)`, read-only.
#[async_trait]
pub trait TaskLines: Send + Sync {
    /// Every task due at or after `from` and before `to`, whatever its progress.
    ///
    /// # Errors
    /// The tasks program could not be reached or sent something unreadable.
    async fn due_between(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<TaskLine>, TasksUnread>;
}
