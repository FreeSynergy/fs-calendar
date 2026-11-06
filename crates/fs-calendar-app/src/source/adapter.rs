//! `CalendarAdapter`: `dyn Calendar` as a `CalendarSource`.
//!
//! Pattern: Adapter over the program's contract, driven with a tokio handle from the interface thread.
//! Provisional: in process, not RPC, until the Cap'n Proto server exists (`DESIGN.md`).
//!
//! Revision: the store's `updated` time and a fingerprint of the content. `Calendar::update` takes no
//! revision, so `save` compares and then writes; this is **not atomic** (another writer can slip in
//! between). A compare-and-set is a wish for the core.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use chrono::Duration;
use fs_calendar_core::time::{Zone, parse_local};
use fs_calendar_core::{Agenda, Calendar, Error, Event, Instance};
use fs_components_workbenches::SourceError;
use fs_components_workbenches::calendar::{Day, Moment};
use fs_store::{ObjectId, StoredObject};
use tokio::runtime::Handle;

use super::items::window;

/// The contract of the calendar program, for the workbench. Do not call it from inside the runtime: it
/// blocks the calling thread until the core has answered.
pub struct CalendarAdapter {
    pub(super) calendar: Arc<dyn Calendar>,
    pub(super) runtime: Handle,
    pub(super) zone: Zone,
    pub(super) frozen: Option<Moment>,
    pub(super) week_start: u8,
}

impl CalendarAdapter {
    /// An adapter over a provider of the contract, driven with `runtime`, showing times in `zone`. The
    /// week starts on Monday.
    pub fn new(calendar: Arc<dyn Calendar>, runtime: Handle, zone: Zone) -> Self {
        Self {
            calendar,
            runtime,
            zone,
            frozen: None,
            week_start: 0,
        }
    }

    /// The clock stands still at `now`, so pictures and tests are the same every time.
    #[must_use]
    pub fn frozen_at(mut self, now: Moment) -> Self {
        self.frozen = Some(now);
        self
    }

    /// The first day of a week: 0 is Monday, 6 is Sunday.
    #[must_use]
    pub fn week_start(mut self, weekday: u8) -> Self {
        self.week_start = weekday % 7;
        self
    }

    pub(super) fn stored(&self, id: ObjectId) -> Result<StoredObject, SourceError> {
        self.runtime
            .block_on(self.calendar.get(id))
            .map_err(|e| reason(&e))?
            .ok_or_else(|| SourceError::not_found("This event no longer exists."))
    }

    /// Events and tasks in the window. A provider without the agenda answers with events alone.
    pub(super) fn agenda(&self, first: Day, last: Day) -> Result<Agenda, SourceError> {
        let (from, to) = window(first, last, &self.zone);
        match self.runtime.block_on(self.calendar.agenda(from, to)) {
            Ok(agenda) => Ok(agenda),
            Err(Error::Unsupported(_)) => self
                .runtime
                .block_on(self.calendar.occurrences(from, to))
                .map(|events| Agenda {
                    events,
                    tasks: Vec::new(),
                })
                .map_err(|e| reason(&e)),
            Err(e) => Err(reason(&e)),
        }
    }

    /// The occurrence `recurrence` of the event `id`: looked for around its own date, or the date its
    /// override moved it to.
    pub(super) fn occurrence(
        &self,
        id: ObjectId,
        event: &Event,
        recurrence: &str,
    ) -> Result<Instance, SourceError> {
        let moved = event
            .recurrence_overrides
            .get(recurrence)
            .and_then(|o| o["start"].as_str());
        let local = parse_local(moved.unwrap_or(recurrence))
            .map_err(|_| SourceError::not_found("This occurrence no longer exists."))?;
        let at = local.and_utc();
        let margin = Duration::days(3);
        let found = self
            .runtime
            .block_on(self.calendar.occurrences(at - margin, at + margin))
            .map_err(|e| reason(&e))?;
        found
            .into_iter()
            .find(|i| i.object == id && i.occurrence.recurrence_id == recurrence)
            .ok_or_else(|| SourceError::not_found("This occurrence no longer exists."))
    }
}

/// What the core said, in the workbench's terms. A store refusing a sensitive event is said plainly. A
/// conflict of the core (a `uid` taken) is no revision conflict: pressing Save again would not help.
pub(super) fn reason(error: &Error) -> SourceError {
    match error {
        Error::Store(fs_store::Error::EncryptionRequired(_)) => SourceError::failed(format!(
            "this event is marked sensitive and this store is not encrypted, so it was refused ({error})"
        )),
        Error::NotFound(_) => SourceError::not_found(error.to_string()),
        Error::Unsupported(_) => SourceError::unsupported(error.to_string()),
        _ => SourceError::failed(error.to_string()),
    }
}

/// The same reason as plain text.
pub(crate) fn reason_text(error: &Error) -> String {
    reason(error).message
}

/// `updated` alone could repeat within the store's clock tick.
pub(super) fn revision_of(object: &StoredObject) -> String {
    let mut hash = DefaultHasher::new();
    object.content.to_string().hash(&mut hash);
    format!(
        "{}/{:x}",
        object.envelope.updated.to_rfc3339(),
        hash.finish()
    )
}

pub(super) fn event_of(object: &StoredObject) -> Result<Event, SourceError> {
    Event::from_value(object.content.clone()).map_err(|e| SourceError::failed(e.to_string()))
}
