//! Validation of Events against JSCalendar 2.0: module map.
//!
//! Pattern: Facade (declarations) — callers use `validate::{issues, validate, ...}`.

mod check;

pub use check::{issues, validate, validate_event};
