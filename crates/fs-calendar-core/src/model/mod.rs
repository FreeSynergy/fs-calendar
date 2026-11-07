//! Typed JSCalendar 2.0 `Event` model: module map.
//!
//! Pattern: Facade (declarations) — callers keep using `model::{Event, Participant, ...}`. What
//! every JSCalendar object shares comes from `fs-jscalendar`; `Event` itself is ours (ADR 0050).
//! Unknown and vendor properties stay verbatim in `extra`, so round trips lose nothing.

mod event;
mod marks;

pub use event::Event;
pub use fs_jscalendar::model::{
    Extra, LEGACY_KEY, Map, NDay, PARTICIPATION_STATUSES, Participant, RecurrenceRule, VENDOR,
    VERSION,
};
pub use marks::{SENSITIVE_KEY, UNCLEAR_SENSITIVE};
