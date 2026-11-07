//! Recurrence expansion at query time: module map.
//!
//! Pattern: Strategy (declarations) — [`RecurrenceEngine`] is the swappable rule engine
//! (`RRuleEngine` uses the `rrule` crate); everything around it (overrides, zones, windows) is shared.

mod early;
mod engine;
mod expand;
mod instance;
mod occurrence;
mod patch;
mod rrule_engine;
mod rrule_string;
mod steps;

pub use early::earliest_start;
pub use engine::RecurrenceEngine;
pub use expand::{
    Damage, Expansion, expand, expand_lenient, expand_with, series_end, series_end_with,
};
pub(crate) use instance::master_value;
pub use occurrence::{Occurrence, first_span, zone_of};
pub use patch::apply_patch;
pub use rrule_engine::RRuleEngine;
pub use rrule_string::rrule_string;
