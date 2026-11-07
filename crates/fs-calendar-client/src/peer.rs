//! The connection: what a frontend holds.
//!
//! Pattern: Adapter — `Calendar` of the core in front of the Cap'n Proto client of `events.capnp`.
//! The thread of the connection is the shared `Worker` of `fs-interfaces`, with the contract in
//! `remote.rs`; the calls in `calls/`; the traits in `calendar_impl.rs`. Interface first (ADR 0043):
//! `connect` and the traits were written down before the calls.

use std::future::Future;
use std::io;
use std::path::Path;

use fs_calendar_core::{Error, Result};
use fs_interfaces::Capabilities;
use fs_interfaces::rpc::client::{Fault, Timeouts, Worker};
use fs_store::Error as StoreError;

use crate::offered::offered;
use crate::remote::Remote;

/// The name of the server in every error text (`calendar server: ...`).
const SERVER: &str = "calendar";

/// The calendar program as seen over its socket. Implements `Calendar` and `RevisedCalendar`.
///
/// Calls from any thread are fine; they queue for the one thread that owns the connection and
/// overlap there. Dropping it closes the connection.
pub struct RpcCalendar {
    worker: Worker<Remote>,
    capabilities: Capabilities,
}

impl RpcCalendar {
    /// Connect to the `fs-calendar-server` that listens on `socket` and ask what it can do.
    ///
    /// Blocks the calling thread until the server has answered; call it before the window opens, not
    /// from inside an async task of a single-threaded runtime.
    ///
    /// # Errors
    /// Nothing listens there, the server runs as another user, it does not answer the contract, or
    /// it stays silent past the connect deadline.
    pub fn connect(socket: &Path) -> io::Result<Self> {
        Self::connect_with(socket, Timeouts::default())
    }

    /// [`connect`](Self::connect) with other deadlines than the defaults.
    ///
    /// # Errors
    /// As for [`connect`](Self::connect).
    pub fn connect_with(socket: &Path, timeouts: Timeouts) -> io::Result<Self> {
        let (worker, capabilities) = Worker::start(SERVER, socket, timeouts)?;
        Ok(Self {
            worker,
            capabilities: offered(&capabilities),
        })
    }

    /// What the server reported when this connection was made, cut to what this client keeps.
    pub(crate) fn report(&self) -> Capabilities {
        self.capabilities.clone()
    }

    /// Run `work` on the connection's thread and wait for its answer. A call that got none (the
    /// connection is gone, the deadline ran out) is a `Backend` error with the text of the `Fault`:
    /// the core has no variant for it.
    pub(crate) async fn call<T, F, Fut>(&self, work: F) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce(Remote) -> Fut + Send + 'static,
        Fut: Future<Output = Result<T>> + 'static,
    {
        let fault = |f: Fault| Error::Store(StoreError::Backend(f.to_string()));
        self.worker.call(work).await.map_err(fault)?
    }
}
