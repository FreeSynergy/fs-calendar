//! `TasksInWindow` and `TaskIntake` over the contract.
//!
//! Pattern: Adapter — each method is one command for the thread of the connection (`call`); the
//! flow of `forward` (known `uid` is skipped, a refused task is only itself lost) is written once
//! here, on top of the calls.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use fs_calendar_core::{DueTask, Forwarded, ImportReport, TaskIntake, TasksError, TasksInWindow};

use crate::calls::{self, Created};
use crate::rpc_tasks::RpcTasks;

#[async_trait]
impl TasksInWindow for RpcTasks {
    async fn due_between(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<DueTask>, TasksError> {
        self.call(move |remote| async move { calls::due_between(&remote.items, from, to).await })
            .await
    }
}

#[async_trait]
impl TaskIntake for RpcTasks {
    async fn import_ics(&self, text: &str) -> Result<ImportReport, TasksError> {
        let text = text.to_string();
        self.call(move |remote| async move { calls::import_ics(&remote.items, &text).await })
            .await
    }

    async fn forward(&self, uid: &str, jscalendar: &str) -> Result<Forwarded, TasksError> {
        let (uid, text) = (uid.to_string(), jscalendar.to_string());
        self.call(move |remote| async move {
            if calls::exists(&remote.items, &uid).await? {
                return Ok(Forwarded::Skipped);
            }
            Ok(match calls::create(&remote.items, &text).await? {
                Created::Id(id) => Forwarded::Created(id),
                Created::Refused(why) => Forwarded::Refused(why),
                Created::Exists => Forwarded::Skipped,
            })
        })
        .await
    }
}
