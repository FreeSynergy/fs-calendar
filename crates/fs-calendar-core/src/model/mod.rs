//! Typed JSCalendar 2.0 model: module map.
//!
//! Pattern: Facade (declarations) — callers keep using `model::{Event, Participant, ...}`.
//! Unknown and vendor properties stay verbatim in `extra`, so round trips lose nothing.

#[macro_use]
mod macros;
mod consts;
mod event;
mod participant;
mod rule;

pub use consts::{Extra, LEGACY_KEY, Map, VENDOR, VERSION};
pub use event::Event;
pub use participant::Participant;
pub use rule::{NDay, RecurrenceRule};
