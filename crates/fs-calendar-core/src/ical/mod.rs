//! iCalendar (RFC 5545) import / export: module map.
//!
//! Pattern: Adapter (declarations) — the files wrap `calcard`; `format::IcsFormat` puts them
//! behind the `Importer` / `Exporter` traits.

mod convert;
mod split;

pub use convert::{ICS_MEDIA_TYPE, parse_ics, to_ics};
pub use split::{IcsPart, split_ics};
