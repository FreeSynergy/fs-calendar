//! Import / export formats behind traits: module map.
//!
//! Pattern: Adapter (declarations) + Factory (`TextFormat` picks a format by media type) — each format file wraps a foreign representation (iCalendar,
//! JSCalendar JSON) in the `Importer` / `Exporter` traits; more formats plug in the same way.

mod ics;
mod jscalendar;
mod media;
mod traits;

pub use ics::IcsFormat;
pub use jscalendar::JsCalendarFormat;
pub use media::{JSCALENDAR_MEDIA_TYPE, TextFormat};
pub use traits::{Exporter, Importer, Parsed, Part, Split};
