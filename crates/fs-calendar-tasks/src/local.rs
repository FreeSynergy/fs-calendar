//! The task program in this process: `TaskItems` of `fs-tasks-core` behind the calendar's traits.
//!
//! Pattern: Adapter — only with the feature `local`, for a program that holds the task store itself
//! (the calendar window does today). The core of the calendar never sees `fs-tasks-core`; this is the
//! one place that does. The tasks leave as their JSCalendar value and their envelope.

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use fs_calendar_core::{DueTask, Forwarded, ImportReport, TaskIntake, TasksError, TasksInWindow};
use fs_tasks_core::{Error, TaskEntry, TaskItems, TaskQuery};

/// `TaskItems` in this process as `TasksInWindow` and `TaskIntake`. Cheap to clone.
#[derive(Clone)]
pub struct LocalTasks(Arc<dyn TaskItems>);

impl LocalTasks {
    /// Put these tasks behind the calendar's traits.
    pub fn new(tasks: Arc<dyn TaskItems>) -> Self {
        Self(tasks)
    }
}

fn failed(e: &Error) -> TasksError {
    TasksError::new(e.to_string())
}

fn due_task(entry: TaskEntry) -> DueTask {
    DueTask {
        id: entry.id,
        envelope: entry.envelope,
        jscalendar: entry.task.to_value(),
    }
}

#[async_trait]
impl TasksInWindow for LocalTasks {
    async fn due_between(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<DueTask>, TasksError> {
        let query = TaskQuery::new().due_between(from, to);
        let entries = self.0.query(&query).await.map_err(|e| failed(&e))?;
        Ok(entries.into_iter().map(due_task).collect())
    }
}

#[async_trait]
impl TaskIntake for LocalTasks {
    async fn import_ics(&self, text: &str) -> Result<ImportReport, TasksError> {
        let t = self.0.import_ics(text).await.map_err(|e| failed(&e))?;
        Ok(ImportReport {
            created: t.created,
            skipped: t.skipped,
            failed: t.failed,
            notes: t.notes,
        })
    }

    async fn forward(&self, uid: &str, jscalendar: &str) -> Result<Forwarded, TasksError> {
        let value: serde_json::Value = match serde_json::from_str(jscalendar) {
            Ok(value) => value,
            Err(e) => return Ok(Forwarded::Refused(e.to_string())),
        };
        let mut task = match fs_tasks_core::Task::from_value(value) {
            Ok(task) => task,
            Err(e) => return Ok(Forwarded::Refused(Error::from(e).to_string())),
        };
        task.normalise();
        if self
            .0
            .get_by_uid(uid)
            .await
            .map_err(|e| failed(&e))?
            .is_some()
        {
            return Ok(Forwarded::Skipped);
        }
        match self.0.create(task).await {
            Ok(entry) => Ok(Forwarded::Created(entry.id)),
            Err(e @ (Error::Invalid(_) | Error::Time(_))) => Ok(Forwarded::Refused(e.to_string())),
            Err(e) => Err(failed(&e)),
        }
    }
}
