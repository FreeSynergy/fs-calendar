//! `RevisedCalendar`: an event together with the revision the server gives it.
//!
//! Pattern: Strategy (a narrow trait beside `Calendar`, so a frontend depends on `dyn
//! RevisedCalendar` and not on [`RpcCalendar`](crate::RpcCalendar)). Why a trait of its own:
//! `StoredObject` has no place for the revision, and the core cannot derive it from the object read
//! here (the wire keeps time in milliseconds, the store in microseconds), so the number from
//! `Event.revision` is handed on as it came. It is the `expected` of
//! [`Calendar::update_checked`](fs_calendar_core::Calendar::update_checked); compare it for equality
//! only.

use async_trait::async_trait;
use fs_calendar_core::{Result, Revision};
use fs_objects::{ObjectId, StoredObject};

/// An event as read, and its revision.
#[derive(Debug, Clone)]
pub struct Revised {
    /// The event.
    pub object: StoredObject,
    /// What the server says the event is now.
    pub revision: Revision,
}

/// Reading an event with its revision.
#[async_trait]
pub trait RevisedCalendar: Send + Sync {
    /// The event and its revision; `None` when there is no such event.
    ///
    /// # Errors
    /// The server could not be reached or refused the call.
    async fn get_revised(&self, id: ObjectId) -> Result<Option<Revised>>;
}
