//! iCalendar (RFC 5545) import / export of events: module map.
//!
//! Pattern: Adapter (declarations) — makes typed events from the shared conversion in
//! `fs-jscalendar`; `format::IcsFormat` puts them behind the `Importer` / `Exporter` traits.

mod convert;
mod zones;

pub use convert::{ICS_MEDIA_TYPE, IcsPart, parse_ics, split_ics, to_ics};
pub use zones::unknown_zone;
