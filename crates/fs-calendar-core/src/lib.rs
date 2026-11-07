//! `fs-calendar-core`: program core of FreeSynergy calendar (no GUI).
//!
//! Events are JSCalendar 2.0 objects inside AS2 `Event` envelopes stored through
//! `fs-store` (ADR 0014, 0019, 0020). Start with the [`Calendar`] trait; [`CalendarRepo`] is its
//! store-backed implementation. Tasks are not ours (ADR 0050): they belong to `fs-tasks` and are
//! shown in the calendar only through its `TaskItems` interface ([`CalendarRepo::with_tasks`],
//! [`CalendarRepo::agenda`]). See `DESIGN.md` for the pattern map.
//!
//! Pattern: Facade — this file only declares modules and re-exports the public API.

#![deny(missing_docs)]
#![warn(clippy::pedantic)]
// Pedantic lints that would only add noise to this crate's small, documented API:
// every fallible fn returns the crate `Result` (documented once on `Error`), results of pure
// getters are used by callers anyway, and the date arithmetic casts are range-checked by jiff.
#![allow(
    clippy::missing_errors_doc,
    clippy::must_use_candidate,
    clippy::module_name_repetitions
)]

mod calendar;
#[cfg(feature = "contract")]
pub mod contract;
mod error;
mod feed;
pub mod format;
pub mod ical;
mod invite;
pub mod limits;
pub mod model;
pub mod recur;
mod repo;
mod revision;
pub mod uid;
pub mod validate;

pub use calendar::Calendar;
pub use error::{Error, Result};
pub use feed::{CalendarChange, CalendarFeed, CalendarListener, Registry, Subscription};
pub use format::{Exporter, IcsFormat, Importer, JsCalendarFormat, TextFormat};
pub use fs_interfaces::{Capabilities, Unsupported};
#[cfg(feature = "contract")]
pub use fs_interfaces::{common_capnp, events_capnp};
pub use fs_jscalendar::upgrade::{Upgraded, upgrade};
pub use fs_jscalendar::{Issue, now_utc, time, upgrade};
pub use invite::{InvitationEntry, ParticipationStatus, Response};
pub use model::{
    Event, Participant, RecurrenceRule, SENSITIVE_KEY, UNCLEAR_SENSITIVE, memory_weight,
};
pub use recur::{Occurrence, RRuleEngine, RecurrenceEngine};
pub use repo::{
    Agenda, CONTENT_TYPE, CONTEXT, CalendarRepo, Damaged, EVENT_TYPE, ImportReport, Instance,
    Window,
};
pub use revision::Revision;
pub use time::{Zone, ZoneSource, parse_utc as utc};
