//! Formats of local and UTC date-times (JSCalendar `LocalDateTime` / `UTCDateTime`).
//!
//! Pattern: none (pure functions) — one parser and one formatter per wire form, so the whole
//! crate agrees on them; UTC index values sort lexicographically in time order.

use chrono::{DateTime, NaiveDateTime, Utc};

use crate::error::{Error, Result};

/// Format of UTC index values: lexicographic order == time order.
pub const UTC_FORMAT: &str = "%Y-%m-%dT%H:%M:%SZ";
/// Far-future marker for series without end.
pub const FOREVER: &str = "9999-12-31T23:59:59Z";

/// Format as `UTCDateTime` (lexicographic order == time order).
pub fn fmt_utc(t: DateTime<Utc>) -> String {
    t.format(UTC_FORMAT).to_string()
}

/// JSCalendar `LocalDateTime` (`2026-11-24T10:00:00`, no zone, no fraction in 2.0).
pub fn parse_local(s: &str) -> Result<NaiveDateTime> {
    if s.len() != 19 {
        return Err(Error::Time(format!("not a LocalDateTime: {s:?}")));
    }
    NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S")
        .map_err(|_| Error::Time(format!("not a LocalDateTime: {s:?}")))
}

/// Format as JSCalendar `LocalDateTime`.
pub fn fmt_local(t: NaiveDateTime) -> String {
    t.format("%Y-%m-%dT%H:%M:%S").to_string()
}

/// `UTCDateTime`: `2026-11-05T09:00:00Z`.
pub fn parse_utc(s: &str) -> Result<DateTime<Utc>> {
    s.strip_suffix('Z')
        .filter(|x| x.len() == 19)
        .and_then(|x| NaiveDateTime::parse_from_str(x, "%Y-%m-%dT%H:%M:%S").ok())
        .map(|n| n.and_utc())
        .ok_or_else(|| Error::Time(format!("not a UTCDateTime: {s:?}")))
}
