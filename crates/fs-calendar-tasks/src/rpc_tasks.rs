//! The connection: what the calendar holds.
//!
//! Pattern: Adapter — `TasksInWindow` and `TaskIntake` in front of the Cap'n Proto client of
//! `tasks.capnp`. The thread of the connection is the shared `Worker` of `fs-interfaces`, with the
//! contract in `remote.rs`; the calls are in `calls.rs`, the traits in `traits_impl.rs`.

use std::future::Future;
use std::io;
use std::path::Path;

use fs_calendar_core::TasksError;
use fs_interfaces::Capabilities;
use fs_interfaces::rpc::client::{Fault, Timeouts, Worker};

use crate::remote::Remote;

/// The name of the server in every error text (`tasks server: ...`).
const SERVER: &str = "tasks";

/// The tasks program as seen over its socket. Implements `TasksInWindow` and `TaskIntake`.
///
/// Calls from any thread are fine; they queue for the one thread that owns the connection and
/// overlap there. Dropping it closes the connection.
pub struct RpcTasks {
    worker: Worker<Remote>,
    capabilities: Capabilities,
}

impl RpcTasks {
    /// Connect to the `fs-tasks-server` that listens on `socket` and ask what it can do.
    ///
    /// Blocks the calling thread until the server has answered; call it before the window opens, not
    /// from inside an async task of a single-threaded runtime.
    ///
    /// # Errors
    /// Nothing listens there, the server runs as another user, or it does not answer the contract.
    pub fn connect(socket: &Path) -> io::Result<Self> {
        Self::connect_with(socket, Timeouts::default())
    }

    /// [`connect`](Self::connect) with other deadlines than the default ones.
    ///
    /// # Errors
    /// As for [`connect`](Self::connect), and the server did not answer in time.
    pub fn connect_with(socket: &Path, timeouts: Timeouts) -> io::Result<Self> {
        let (worker, capabilities) = Worker::start(SERVER, socket, timeouts)?;
        Ok(Self {
            worker,
            capabilities,
        })
    }

    /// What the server reported when this connection was made. A snapshot: the server is not asked again.
    pub fn capabilities(&self) -> &Capabilities {
        &self.capabilities
    }

    /// Run `work` on the connection's thread and wait for its answer. A call that got none (the
    /// connection is gone, the job ended, the deadline ran out) is a [`TasksError`] with the text of
    /// the `Fault`.
    pub(crate) async fn call<T, F, Fut>(&self, work: F) -> Result<T, TasksError>
    where
        T: Send + 'static,
        F: FnOnce(Remote) -> Fut + Send + 'static,
        Fut: Future<Output = Result<T, TasksError>> + 'static,
    {
        let fault = |f: Fault| TasksError::new(f.to_string());
        self.worker.call(work).await.map_err(fault)?
    }
}
