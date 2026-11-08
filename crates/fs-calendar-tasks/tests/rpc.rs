//! The task program over `tasks.capnp` (`RpcTasks`) keeps what the calendar needs of it, against a real
//! `fs-tasks-server` on a real Unix socket. Pages of two, so the walk over pages is what is tested.

mod common;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::Utc;
use fs_calendar_core::{TaskIntake, TasksInWindow};
use fs_calendar_tasks::RpcTasks;
use fs_interfaces::rpc::{bind_socket, serve};
use fs_tasks_core::TaskRepo;
use fs_tasks_server::TasksServer;

/// A server on its own thread (a Cap'n Proto server is not `Send`), stopped when this is dropped.
struct Running {
    socket: PathBuf,
    stop: Option<tokio::sync::oneshot::Sender<()>>,
    thread: Option<std::thread::JoinHandle<()>>,
    _dir: tempfile::TempDir,
}

impl Running {
    fn start(program: TaskRepo) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let socket = dir.path().join("tasks.sock");
        let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
        let (ready, up) = std::sync::mpsc::channel();
        let path = socket.clone();
        let thread = std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            let local = tokio::task::LocalSet::new();
            local.block_on(&runtime, async move {
                let repo = Arc::new(program);
                let root = TasksServer::new(repo.clone(), repo)
                    .with_page_size(2)
                    .into_client();
                let bound = bind_socket(&path).await.unwrap();
                ready.send(()).unwrap();
                serve(bound, root.client, async {
                    let _ = stopped.await;
                })
                .await
                .unwrap();
            });
        });
        up.recv().unwrap();
        Self {
            socket,
            stop: Some(stop),
            thread: Some(thread),
            _dir: dir,
        }
    }

    fn socket(&self) -> &Path {
        &self.socket
    }

    fn stop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(thread) = self.thread.take() {
            thread.join().unwrap();
        }
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        self.stop();
    }
}

async fn connect(socket: PathBuf) -> RpcTasks {
    tokio::task::spawn_blocking(move || RpcTasks::connect(&socket).unwrap())
        .await
        .unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn rpc_tasks_behave() {
    let (_store, program) = common::task_program().await;
    let server = Running::start(program.clone());
    let client = Arc::new(connect(server.socket().to_path_buf()).await);
    let reported = client.capabilities();
    assert!(
        reported
            .entries()
            .iter()
            .any(|e| e.capability == "task.query" && e.supported),
        "the server's report is kept: {reported:?}"
    );
    common::behaves(client.clone(), client, program).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_server_that_is_gone_is_an_error_not_an_empty_list() {
    let (_store, program) = common::task_program().await;
    let mut server = Running::start(program);
    let client = connect(server.socket().to_path_buf()).await;
    server.stop();
    let now = Utc::now();
    let failed = client.due_between(now, now).await.unwrap_err();
    assert!(failed.0.contains("tasks server"), "{failed}");
    let failed = client
        .forward("x", r#"{"@type":"Task","uid":"x"}"#)
        .await
        .unwrap_err();
    assert!(failed.0.contains("tasks server"), "{failed}");
}
