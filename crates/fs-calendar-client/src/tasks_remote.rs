//! What the client takes from the root of the tasks contract.
//!
//! Pattern: Strategy — the handshake of the tasks connection and the one capability a call uses; the
//! thread and the deadlines are the shared `Worker` of `fs-interfaces`.

use fs_interfaces::Capabilities;
use fs_interfaces::rpc::client::Remote;
use fs_interfaces::tasks_capnp::{items, tasks};

/// The capability of the tasks server a call may use.
#[derive(Clone)]
pub(crate) struct TasksRemote {
    pub(crate) items: items::Client,
}

impl Remote for TasksRemote {
    type Root = tasks::Client;

    async fn open(root: tasks::Client) -> Result<(Self, Capabilities), capnp::Error> {
        let report = root.support_report_request().send().promise.await?;
        let capabilities = Capabilities::read(report.get()?.get_report()?)?;
        let items = root.items_request().send().promise.await?;
        let items = items.get()?.get_items()?;
        Ok((Self { items }, capabilities))
    }
}
