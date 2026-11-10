//! `EventBackend` itself: the struct, its helpers and the `Backend` impl.
//!
//! Pattern: Adapter (`Backend` over the `Calendar` trait); see `mod.rs` for the whole picture.

use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, Mutex, PoisonError};

use fs_admin_frontend::{Backend, BackendError, Outcome, Record, Values};
use fs_calendar_core::{Calendar, Instance, Revision, support};
use fs_objects::ObjectId;
use fs_types::{DescribesType, TypeDescription};

use tokio::runtime::Handle;

/// The AS2 id of an event: `urn:uuid:<id>`; an occurrence of a series adds `#<recurrence id>`.
#[must_use]
pub fn iri(id: ObjectId) -> String {
    format!("urn:uuid:{id}")
}

/// The id of one instance in a list or an agenda: the event, and for an instance of a series `#<recurrence id>`.
#[must_use]
pub fn instance_id(i: &Instance) -> String {
    match &i.occurrence.event.recurrence_id {
        Some(r) => format!("{}#{r}", iri(i.object)),
        None => iri(i.object),
    }
}

/// The backend of the `Event` type.
pub struct EventBackend {
    pub(crate) calendar: Arc<dyn Calendar>,
    pub(crate) handle: Handle,
    pub(crate) description: TypeDescription,
    revisions: Mutex<HashMap<ObjectId, Revision>>,
}

impl EventBackend {
    /// Serve `calendar`; its futures run on `handle`.
    #[must_use]
    pub fn new(calendar: Arc<dyn Calendar>, handle: Handle) -> Self {
        Self {
            calendar,
            handle,
            description: fs_calendar_core::Event::describe(),
            revisions: Mutex::new(HashMap::new()),
        }
    }

    pub(crate) fn block<T>(&self, f: impl Future<Output = T>) -> T {
        self.handle.block_on(f)
    }

    pub(crate) fn remember(&self, id: ObjectId, revision: Revision) {
        self.revisions
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(id, revision);
    }

    pub(crate) fn revision(&self, id: ObjectId) -> Option<Revision> {
        self.revisions
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&id)
            .copied()
    }
}

impl Backend for EventBackend {
    fn list(&self, query: &Values) -> Result<Vec<Record>, BackendError> {
        self.find(query)
    }

    fn run(&self, name: &str, input: &Values) -> Result<Outcome, BackendError> {
        if self.description.find_command(name).is_none() {
            return Err(BackendError::NotFound);
        }
        // The last guard: the frontend of rev a528ea2 cannot show a locked control, so it may still send one.
        if !support::is_supported(name) {
            return Err(BackendError::Unavailable(format!("unsupported: {name}")));
        }
        self.command(name, input)
    }
}
