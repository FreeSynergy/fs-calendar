//! The current time in the JSCalendar `UTCDateTime` form.
//!
//! Pattern: none (pure helper) — one place for the timestamp format used by objects and the repository.

/// Current time as a JSCalendar `UTCDateTime`.
pub fn now_utc() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}
