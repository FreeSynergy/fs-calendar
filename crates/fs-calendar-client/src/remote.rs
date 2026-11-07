//! What the client takes from the root of the events contract.
//!
//! Pattern: Strategy — the one part of the connection that is the calendar program's own: the
//! handshake and the capability a call uses. The thread, the queue and the deadlines are the shared
//! `Worker` of `fs-interfaces`.

use fs_interfaces::Capabilities;
use fs_interfaces::events_capnp::{events, items};
use fs_interfaces::rpc::client::Remote as Contract;

/// The capability of the server a call may use.
#[derive(Clone)]
pub(crate) struct Remote {
    pub(crate) items: items::Client,
}

impl Contract for Remote {
    type Root = events::Client;

    async fn open(root: events::Client) -> Result<(Self, Capabilities), capnp::Error> {
        let report = root.support_report_request().send().promise.await?;
        let capabilities = Capabilities::read(report.get()?.get_report()?)?;
        let items = root.items_request().send().promise.await?;
        let items = items.get()?.get_items()?;
        Ok((Self { items }, capabilities))
    }
}
