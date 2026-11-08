//! `RemoteCalendar`: `Calendar` over the Cap'n Proto socket of the calendar server (ADR 0063).
//!
//! Pattern: Proxy — a Cap'n Proto client lives on one thread, so a worker thread owns the connection
//! and the proxy hands it calls through a channel; the window thread waits for the answer like for the
//! in-process core.

use std::path::Path;
use std::thread;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use fs_calendar_core::{Calendar, Capabilities, Error, Event, Instance, Result, Window};
use fs_objects::{ObjectId, StoredObject};
use tokio::sync::{mpsc, oneshot};

use super::worker::{Call, Reply, Revisions, worker};

/// The calendar server behind a socket, as a provider of [`Calendar`].
pub struct RemoteCalendar {
    calls: mpsc::UnboundedSender<Call>,
}

impl RemoteCalendar {
    /// Connect to the server on `socket`.
    ///
    /// # Errors
    /// The text for the person when nothing answers on the socket.
    pub fn connect(socket: &Path) -> std::result::Result<Self, String> {
        let (calls, inbox) = mpsc::unbounded_channel();
        let (ready, started) = std::sync::mpsc::channel();
        let socket = socket.to_path_buf();
        let revisions = Revisions::default();
        thread::Builder::new()
            .name("calendar-rpc".into())
            .spawn(move || worker(&socket, inbox, &ready, &revisions))
            .map_err(|e| e.to_string())?;
        started.recv().map_err(|e| e.to_string())??;
        Ok(Self { calls })
    }

    async fn ask<T>(&self, make: impl FnOnce(Reply<T>) -> Call) -> Result<T> {
        let (reply, answer) = oneshot::channel();
        self.calls.send(make(reply)).map_err(|_| gone())?;
        answer.await.map_err(|_| gone())?
    }
}

fn gone() -> Error {
    Error::Stored("the calendar server went away".into())
}

#[async_trait]
impl Calendar for RemoteCalendar {
    fn capabilities(&self) -> Capabilities {
        let mut caps = Capabilities::new("fs-calendar-remote");
        for id in [
            "event.create",
            "event.update",
            "event.delete",
            "event.read",
            "event.occurrences",
        ] {
            caps.supported(id);
        }
        caps.unsupported("event.import", "use the command line or the local mode");
        caps.unsupported("event.export", "use the command line or the local mode");
        caps.unsupported("invite.create", "not asked over the socket yet");
        caps
    }
    async fn get(&self, id: ObjectId) -> Result<Option<StoredObject>> {
        self.ask(|reply| Call::Get(id, reply)).await
    }
    async fn get_event(&self, id: ObjectId) -> Result<Option<Event>> {
        match self.get(id).await? {
            Some(o) => Ok(Some(Event::from_value(o.content)?)),
            None => Ok(None),
        }
    }
    async fn get_by_uid(&self, uid: &str) -> Result<Option<StoredObject>> {
        let uid = uid.to_string();
        self.ask(|reply| Call::ByUid(uid, reply)).await
    }
    async fn list_events(&self) -> Result<Vec<StoredObject>> {
        self.ask(Call::List).await
    }
    async fn occurrences(&self, from: DateTime<Utc>, to: DateTime<Utc>) -> Result<Vec<Instance>> {
        Ok(self.window(from, to).await?.instances)
    }
    async fn window(&self, from: DateTime<Utc>, to: DateTime<Utc>) -> Result<Window> {
        self.ask(|reply| Call::Window(from, to, reply)).await
    }
    async fn create(&self, event: Event) -> Result<StoredObject> {
        let text = event.to_value().to_string();
        self.ask(|reply| Call::Create(text, reply)).await
    }
    async fn update(&self, id: ObjectId, new: Event) -> Result<StoredObject> {
        let text = new.to_value().to_string();
        self.ask(|reply| Call::Update(id, text, reply)).await
    }
    async fn delete(&self, id: ObjectId) -> Result<()> {
        self.ask(|reply| Call::Delete(id, reply)).await
    }
}
