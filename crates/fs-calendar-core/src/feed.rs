//! The [`CalendarFeed`] contract: live changes of events (`event.subscribe`).
//!
//! Pattern: Observer — a [`CalendarListener`] subscribes and is told about every change; dropping
//! the [`Subscription`] ends it (as in `common.capnp`). [`Registry`] is the reusable subject.

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use fs_objects::ObjectId;

/// What happened to an event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CalendarChange {
    /// An event was created (also by an import).
    Created(ObjectId),
    /// An event was changed (update, invitation, answer).
    Updated(ObjectId),
    /// An event was deleted.
    Deleted(ObjectId),
}

/// Receives changes. Called on the thread that made the change: keep it short, hand work off.
pub trait CalendarListener: Send + Sync {
    /// An event changed.
    fn changed(&self, change: &CalendarChange);
}

/// Live updates of events.
pub trait CalendarFeed: Send + Sync {
    /// Tell `listener` about every change until the returned [`Subscription`] is dropped.
    fn subscribe(&self, listener: Arc<dyn CalendarListener>) -> Subscription;
}

type Entries = Vec<(u64, Arc<dyn CalendarListener>)>;

/// The subject of the Observer: holds the listeners and notifies them.
#[derive(Default)]
pub struct Registry {
    next: Mutex<u64>,
    listeners: Mutex<Entries>,
}

impl Registry {
    /// Add a listener; the subscription removes it again.
    pub fn add(self: &Arc<Self>, listener: Arc<dyn CalendarListener>) -> Subscription {
        let mut next = self.next.lock().unwrap_or_else(PoisonError::into_inner);
        *next += 1;
        let id = *next;
        self.entries().push((id, listener));
        Subscription {
            id,
            registry: Arc::clone(self),
        }
    }

    /// Tell every listener.
    pub fn notify(&self, change: &CalendarChange) {
        let listeners: Vec<_> = self.entries().iter().map(|(_, l)| l.clone()).collect();
        for l in listeners {
            l.changed(change);
        }
    }

    fn entries(&self) -> MutexGuard<'_, Entries> {
        self.listeners
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }
}

/// Keeps a subscription alive; dropping it unsubscribes.
#[must_use = "dropping the subscription ends it"]
pub struct Subscription {
    id: u64,
    registry: Arc<Registry>,
}

impl Drop for Subscription {
    fn drop(&mut self) {
        let id = self.id;
        self.registry.entries().retain(|(i, _)| *i != id);
    }
}
