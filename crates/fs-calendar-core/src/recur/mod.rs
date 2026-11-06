//! Recurrence expansion at query time: module map.
//!
//! Pattern: Strategy (declarations) — [`RecurrenceEngine`] is the swappable rule engine
//! (`RRuleEngine` uses the `rrule` crate); everything around it (overrides, zones, windows) is shared.

mod engine;
mod expand;
mod occurrence;
mod patch;
mod rrule_engine;
mod rrule_string;

pub use engine::RecurrenceEngine;
pub use expand::{expand, expand_with, series_end, series_end_with};
pub use occurrence::{Occurrence, first_span, zone_of};
pub use patch::apply_patch;
pub use rrule_engine::RRuleEngine;
pub use rrule_string::rrule_string;
