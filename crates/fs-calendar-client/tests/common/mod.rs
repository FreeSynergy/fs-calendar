//! Shared set-up of the tests: the real calendar server on a real Unix socket over the real store, on
//! a thread of its own, and the client connected to it; for the tasks the real tasks server the same
//! way. Nothing here stands in for either.
#![allow(dead_code)] // each test file uses a different subset

use std::path::PathBuf;
use std::sync::Arc;

use fs_calendar_client::{RpcCalendar, RpcTaskLines};
use fs_calendar_core::{Calendar, CalendarFeed, CalendarRepo, Event};
use fs_calendar_server::CalendarServer;
use fs_interfaces::rpc::{bind_socket, serve};
use fs_objects::{OpenOptions, Store, TursoStore};
use fs_tasks_core::{Task, TaskFeed, TaskItems, TaskRepo};
use fs_tasks_server::TasksServer;
use serde_json::{Value, json};
use tokio::sync::oneshot;

pub const ME: &str = "https://local.invalid/users/owner";

/// A thread that serves until it is dropped.
struct Thread {
    stop: Option<oneshot::Sender<()>>,
    thread: Option<std::thread::JoinHandle<()>>,
    _dir: tempfile::TempDir,
}

impl Thread {
    fn start<F>(name: &str, root: F) -> (Self, PathBuf)
    where
        F: FnOnce() -> capnp::capability::Client + Send + 'static,
    {
        let dir = tempfile::tempdir().unwrap();
        let socket = dir.path().join(name);
        let (stop, stopped) = oneshot::channel::<()>();
        let (ready, listening) = std::sync::mpsc::channel();
        let path = socket.clone();
        let thread = std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            tokio::task::LocalSet::new().block_on(&runtime, async move {
                let bound = bind_socket(&path).await.unwrap();
                let client = root();
                ready.send(()).unwrap();
                serve(bound, client, async {
                    let _ = stopped.await;
                })
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

    fn stop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for Thread {
    fn drop(&mut self) {
        self.stop();
    }
}

/// A calendar server until this is dropped.
pub struct Served {
    pub socket: PathBuf,
    /// The provider behind the server, for set-up and for comparing (never what is under test).
    pub repo: Arc<CalendarRepo>,
    pub store: Arc<dyn Store>,
    thread: Thread,
}

impl Served {
    /// Over a fresh in-memory store that is encrypted.
    pub async fn start() -> Self {
        let store: Arc<dyn Store> = Arc::new(TursoStore::open_in_memory().await.unwrap());
        Self::over(store)
    }

    /// Over a plain store, which refuses what is marked sensitive.
    pub async fn start_plain() -> Self {
        let store: Arc<dyn Store> = Arc::new(
            TursoStore::open_with(":memory:", OpenOptions::default())
                .await
                .unwrap(),
        );
        Self::over(store)
    }

    fn over(store: Arc<dyn Store>) -> Self {
        let repo = Arc::new(CalendarRepo::new(store.clone(), ME));
        let provider = repo.clone();
        let (thread, socket) = Thread::start("events.sock", move || {
            let calendar: Arc<dyn Calendar> = provider.clone();
            let feed: Arc<dyn CalendarFeed> = provider;
            CalendarServer::new(calendar, feed).into_client().client
        });
        Self {
            socket,
            repo,
            store,
            thread,
        }
    }

    /// A client of this server.
    pub fn client(&self) -> RpcCalendar {
        RpcCalendar::connect(&self.socket).unwrap()
    }

    /// Stop the server; clients stay and find it gone.
    pub fn stop(&mut self) {
        self.thread.stop();
    }
}

/// A tasks server until this is dropped.
pub struct TasksServed {
    pub socket: PathBuf,
    pub repo: Arc<TaskRepo>,
    thread: Thread,
}

impl TasksServed {
    pub async fn start() -> Self {
        let store: Arc<dyn Store> = Arc::new(TursoStore::open_in_memory().await.unwrap());
        let repo = Arc::new(TaskRepo::new(store, ME));
        let provider = repo.clone();
        let (thread, socket) = Thread::start("tasks.sock", move || {
            let items: Arc<dyn TaskItems> = provider.clone();
            let feed: Arc<dyn TaskFeed> = provider;
            TasksServer::new(items, feed).into_client().client
        });
        Self {
            socket,
            repo,
            thread,
        }
    }

    pub fn client(&self) -> RpcTaskLines {
        RpcTaskLines::connect(&self.socket).unwrap()
    }

    pub fn stop(&mut self) {
        self.thread.stop();
    }

    /// A task due at this local time in Berlin.
    pub async fn add(&self, title: &str, due: Option<&str>) -> String {
        let mut task = Task::new(title);
        task.due = due.map(str::to_string);
        task.time_zone = due.map(|_| "Europe/Berlin".to_string());
        TaskItems::create(self.repo.as_ref(), task)
            .await
            .unwrap()
            .id
            .to_string()
    }
}

/// An event of one hour in Berlin, as JSON.
pub fn json(uid: &str, title: &str, start: &str) -> Value {
    json!({"@type":"Event","version":"2.0","uid":uid,"title":title,"start":start,
        "timeZone":"Europe/Berlin","duration":"PT1H"})
}

pub fn event(uid: &str, title: &str, start: &str) -> Event {
    Event::from_value(json(uid, title, start)).unwrap()
}
