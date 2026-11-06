//! The calendar view that shows tasks next to events.
//!
//! Pattern: Facade over two interfaces — events come from the calendar itself, tasks only from
//! the `TaskItems` interface of `fs-tasks` (ADR 0050); no Task model of our own.

use chrono::{DateTime, Utc};
use fs_tasks_core::TaskQuery;

use super::handle::CalendarRepo;
use super::types::Agenda;
use crate::error::Result;

impl CalendarRepo {
    /// Event instances overlapping `[from, to)` and, if a task source is attached
    /// ([`CalendarRepo::with_tasks`]), the tasks due in `[from, to)`.
    pub async fn agenda(&self, from: DateTime<Utc>, to: DateTime<Utc>) -> Result<Agenda> {
        let events = self.occurrences(from, to).await?;
        let tasks = match &self.tasks {
            Some(tasks) => tasks.query(&TaskQuery::new().due_between(from, to)).await?,
            None => vec![],
        };
        Ok(Agenda { events, tasks })
    }
}
