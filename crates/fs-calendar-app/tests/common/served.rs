//! The calendar server on a thread of its own, on a real Unix socket.
//!
//! Pattern: RAII / Guard — the server runs until this is dropped.

use std::path::PathBuf;
use std::sync::Arc;

use fs_calendar_core::{Calendar, CalendarFeed, CalendarRepo};
use fs_calendar_server::CalendarServer;
use fs_interfaces::rpc::{bind_socket, serve};
use tokio::sync::oneshot;

/// A server until dropped.
pub struct Served {
    stop: Option<oneshot::Sender<()>>,
    thread: Option<std::thread::JoinHandle<()>>,
    _dir: tempfile::TempDir,
}

impl Served {
    /// Serve `repo`; answers the socket it listens on.
    pub fn start(repo: &Arc<CalendarRepo>) -> (Self, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let socket = dir.path().join("events.sock");
        let (stop, stopped) = oneshot::channel::<()>();
        let (ready, listening) = std::sync::mpsc::channel();
        let (path, provider) = (socket.clone(), repo.clone());
        let thread = std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            tokio::task::LocalSet::new().block_on(&runtime, async move {
                let bound = bind_socket(&path).await.unwrap();
                let calendar: Arc<dyn Calendar> = provider.clone();
                let feed: Arc<dyn CalendarFeed> = provider;
                let root = CalendarServer::new(calendar, feed);
                ready.send(()).unwrap();
                let shutdown = async {
                    let _ = stopped.await;
                };
                serve(bound, root.into_client().client, shutdown)
                    .await
                    .unwrap();
            });
        });
        listening.recv().unwrap();
        let served = Self {
            stop: Some(stop),
            thread: Some(thread),
            _dir: dir,
        };
        (served, socket)
    }
}

impl Drop for Served {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
