//! What the client takes from the root of the tasks contract.
//!
//! Pattern: Strategy — the one part of the connection that is the contract's own: the handshake and
//! the capability a call may use. The thread, the queue and the deadlines are the shared `Worker` of
//! `fs-interfaces`.

use fs_interfaces::Capabilities;
use fs_interfaces::rpc::client::Remote as Contract;
use fs_interfaces::tasks_capnp::{items, tasks};

/// The capabilities of the server a call may use.
#[derive(Clone)]
pub(crate) struct Remote {
    pub(crate) items: items::Client,
}

impl Contract for Remote {
    type Root = tasks::Client;

    async fn open(root: tasks::Client) -> Result<(Self, Capabilities), capnp::Error> {
        let report = root.support_report_request().send().promise.await?;
        let capabilities = Capabilities::read(report.get()?.get_report()?)?;
        let items = root.items_request().send().promise.await?;
        Ok((
            Self {
                items: items.get()?.get_items()?,
            },
            capabilities,
        ))
    }
}
