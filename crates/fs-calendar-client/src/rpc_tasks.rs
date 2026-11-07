//! `RpcTaskLines`: the tasks of a day from the tasks program over its socket.
//!
//! Pattern: Adapter — `TaskLines` in front of `Items.query` of `tasks.capnp`; the thread is the
//! shared `Worker`. Read only: no other method of the tasks contract is called. A failure is said in
//! words as `TasksUnread`; the calendar decides nothing by them.

use std::io;
use std::path::Path;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use fs_interfaces::rpc::client::problems::Failure;
use fs_interfaces::rpc::client::{Timeouts, Worker};
use fs_interfaces::rpc::{read_envelope, to_millis};
use fs_interfaces::tasks_capnp::items::Client;
use serde_json::Value;

use crate::task_lines::{TaskLine, TaskLines, TasksUnread};
use crate::tasks_remote::TasksRemote;

/// The tasks program as seen over its socket.
pub struct RpcTaskLines {
    worker: Worker<TasksRemote>,
}

impl RpcTaskLines {
    /// Connect to the `fs-tasks-server` that listens on `socket`.
    ///
    /// # Errors
    /// Nothing listens there, or the server does not answer the tasks contract.
    pub fn connect(socket: &Path) -> io::Result<Self> {
        Self::connect_with(socket, Timeouts::default())
    }

    /// [`connect`](Self::connect) with other deadlines.
    ///
    /// # Errors
    /// As for [`connect`](Self::connect).
    pub fn connect_with(socket: &Path, timeouts: Timeouts) -> io::Result<Self> {
        let (worker, _) = Worker::start("tasks", socket, timeouts)?;
        Ok(Self { worker })
    }
}

fn said(e: &capnp::Error) -> TasksUnread {
    TasksUnread(match Failure::of(e) {
        Failure::Problem(p) => format!("tasks server ({}): {}", p.status, p.detail),
        Failure::Unimplemented => "tasks server: it does not know this method".to_string(),
        Failure::Other(text) => format!("tasks server: {text}"),
    })
}

fn unreadable(e: impl std::fmt::Display) -> TasksUnread {
    TasksUnread(format!("tasks server: it sent something unreadable: {e}"))
}

/// `query` for the tasks due in `[from, to)`, every page.
async fn query(
    c: &Client,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> Result<Vec<TaskLine>, TasksUnread> {
    let mut found: Vec<TaskLine> = Vec::new();
    let mut cursor: Vec<u8> = Vec::new();
    loop {
        let before = found.len();
        let mut req = c.query_request();
        let mut filter = req.get().init_filter();
        filter
            .reborrow()
            .init_due_from()
            .set_unix_millis(to_millis(from));
        filter.init_due_to().set_unix_millis(to_millis(to));
        req.get().init_cursor().set_opaque(&cursor);
        let answer = req.send().promise.await.map_err(|e| said(&e))?;
        let page = answer
            .get()
            .and_then(|r| r.get_page())
            .map_err(unreadable)?;
        for item in page.get_items().map_err(unreadable)? {
            let envelope = item
                .get_envelope()
                .and_then(read_envelope)
                .map_err(unreadable)?;
            let text = item.get_jscalendar().map_err(unreadable)?;
            let content: Value =
                serde_json::from_str(text.to_str().map_err(unreadable)?).map_err(unreadable)?;
            found.push(TaskLine {
                id: envelope.id,
                name: envelope.name,
                content,
            });
        }
        cursor = page
            .get_next()
            .and_then(|n| n.get_opaque())
            .map_err(unreadable)?
            .to_vec();
        if cursor.is_empty() {
            return Ok(found);
        }
        if found.len() == before {
            return Err(unreadable("an empty page that names a next one"));
        }
    }
}

#[async_trait]
impl TaskLines for RpcTaskLines {
    async fn due_between(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<TaskLine>, TasksUnread> {
        let fault =
            |f: fs_interfaces::rpc::client::Fault| TasksUnread(format!("tasks server: {f}"));
        self.worker
            .call(move |r| async move { query(&r.items, from, to).await })
            .await
            .map_err(fault)?
    }
}
