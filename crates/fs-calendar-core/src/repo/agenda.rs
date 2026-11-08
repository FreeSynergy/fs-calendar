//! The calendar view that shows tasks next to events.
//!
//! Pattern: Facade over two interfaces — events come from the calendar itself, tasks only from
//! [`TasksInWindow`](crate::TasksInWindow) (ADR 0050); no Task model of our own.

use super::handle::CalendarRepo;
use super::types::{Agenda, Window};
use crate::error::Result;
use chrono::{DateTime, Utc};

impl CalendarRepo {
    /// Event instances overlapping `[from, to)` and, if a task source is attached
    /// ([`CalendarRepo::with_tasks`]), the tasks due in `[from, to)`.
    pub async fn agenda(&self, from: DateTime<Utc>, to: DateTime<Utc>) -> Result<Agenda> {
        let Window { instances, damaged } = self.window(from, to).await?;
        let tasks = match &self.tasks {
            Some(tasks) => tasks.due_between(from, to).await?,
            None => vec![],
        };
        Ok(Agenda {
            events: instances,
            tasks,
            damaged,
        })
    }
}
