//! [`CalendarObject`]: either component the calendar manages.
//!
//! Pattern: Adapter (sum type) — one enum lets the repository treat events and tasks uniformly
//! (uid, title, participants) while keeping their own types.

use serde_json::Value;

use super::consts::Map;
use super::event::{Event, Task};
use super::participant::Participant;
use crate::error::{Error, Result};

/// Either JSCalendar component we manage.
#[derive(Clone, Debug, PartialEq)]
pub enum CalendarObject {
    /// An event.
    Event(Event),
    /// A task.
    Task(Task),
}

impl CalendarObject {
    /// Dispatch on `@type` (`Event` / `Task`).
    pub fn from_value(v: Value) -> Result<Self> {
        match v.get("@type").and_then(Value::as_str) {
            Some("Event") => Ok(Self::Event(Event::from_value(v)?)),
            Some("Task") => Ok(Self::Task(Task::from_value(v)?)),
            other => Err(Error::Invalid(vec![crate::Issue {
                path: "/@type".into(),
                message: format!("expected \"Event\" or \"Task\", got {other:?}"),
            }])),
        }
    }
    /// The object as a JSON value (the stored content).
    ///
    /// # Panics
    /// Never in practice: the object holds only JSON-compatible data.
    pub fn to_value(&self) -> Value {
        match self {
            Self::Event(e) => e.to_value(),
            Self::Task(t) => t.to_value(),
        }
    }
    /// The `uid` of the object.
    pub fn uid(&self) -> &str {
        match self {
            Self::Event(e) => &e.uid,
            Self::Task(t) => &t.uid,
        }
    }
    /// The display title (a placeholder if untitled).
    pub fn title(&self) -> String {
        match self {
            Self::Event(e) => e.display_title(),
            Self::Task(t) => t.display_title(),
        }
    }
    /// `"Event"` or `"Task"`.
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Event(_) => "Event",
            Self::Task(_) => "Task",
        }
    }
    /// The participants, mutable.
    pub fn participants_mut(&mut self) -> &mut Map<Participant> {
        match self {
            Self::Event(e) => &mut e.participants,
            Self::Task(t) => &mut t.participants,
        }
    }
    /// The participants.
    pub fn participants(&self) -> &Map<Participant> {
        match self {
            Self::Event(e) => &e.participants,
            Self::Task(t) => &t.participants,
        }
    }
}

impl From<Event> for CalendarObject {
    fn from(e: Event) -> Self {
        Self::Event(e)
    }
}
impl From<Task> for CalendarObject {
    fn from(t: Task) -> Self {
        Self::Task(t)
    }
}
