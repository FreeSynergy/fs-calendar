//! `fs-calendar-core`: program core of FreeSynergy calendar (no GUI).
//!
//! Events and tasks are JSCalendar 2.0 objects inside AS2 `Event` envelopes stored through
//! `fs-store` (ADR 0014, 0019, 0020). See `DESIGN.md`.

mod error;
pub mod ical;
pub mod model;
pub mod recur;
mod repo;
pub mod time;
pub mod upgrade;
pub mod validate;

pub use error::{Error, Issue, Result};
pub use model::{CalendarObject, Event, Participant, RecurrenceRule, Task};
pub use recur::Occurrence;
pub use repo::{
    CONTENT_TYPE, CONTEXT, CalendarRepo, EVENT_TYPE, ImportReport, Instance, InvitationEntry, Response, TASK_TYPE, utc,
};
pub use time::Zone;
pub use upgrade::{Upgraded, upgrade};

/// Current time as a JSCalendar `UTCDateTime`.
pub fn now_utc() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}
