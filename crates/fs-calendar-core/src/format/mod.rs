//! Import / export formats behind traits: module map.
//!
//! Pattern: Adapter (declarations) — each format file wraps a foreign representation (iCalendar,
//! JSCalendar JSON) in the `Importer` / `Exporter` traits; more formats plug in the same way.

mod ics;
mod jscalendar;
mod traits;

pub use ics::IcsFormat;
pub use jscalendar::JsCalendarFormat;
pub use traits::{Exporter, Importer, Parsed, Part, Split};
