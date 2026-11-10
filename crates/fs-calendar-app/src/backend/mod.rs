//! `EventBackend`: the records and commands of the `Event` description over the calendar's own interface.
//!
//! Pattern: Adapter (`Backend` of `fs-admin-frontend` over the `Calendar` trait, ADR 0043) — it depends on the trait,
//! never on `CalendarRepo`, so the same code serves an in-process repository and a fake. The frontend is synchronous,
//! the core is not: calls run on the runtime of the program (`Handle::block_on`, the window thread is not part of it).
//! Revisions read by `list` are kept, so a `change` is an `updateIf` against what the person saw.

mod commands;
mod errors;
mod find;
mod record;
mod transfer;

use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, Mutex, PoisonError};

use fs_admin_frontend::{Backend, BackendError, Outcome, Record, Values};
use fs_calendar_core::{Calendar, Revision, support};
use fs_objects::ObjectId;
use fs_types::{DescribesType, TypeDescription};
use tokio::runtime::Handle;

pub use record::{apply, to_values};

/// The AS2 id of an event: `urn:uuid:<id>`; an occurrence of a series adds `#<recurrence id>`.
#[must_use]
pub fn iri(id: ObjectId) -> String {
    format!("urn:uuid:{id}")
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
