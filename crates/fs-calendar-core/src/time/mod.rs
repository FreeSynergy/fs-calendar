//! Local date-times, ISO 8601 durations and IANA time zones: module map.
//!
//! Pattern: Facade (declarations) — callers keep using `time::Zone`, `time::parse_local`, ...;
//! the zone database behind them is a [`ZoneSource`] strategy.

mod bundled;
mod dur;
mod format;
mod select;
mod source;
mod system;
mod zone;

pub use bundled::BundledTzdb;
pub use dur::Dur;
pub use format::{FOREVER, UTC_FORMAT, fmt_local, fmt_utc, parse_local, parse_utc};
pub use select::{detect, newest, tzdb_source, tzdb_version};
pub use source::{TzSource, ZoneSource};
pub use system::SystemTzdb;
pub use zone::{IanaZone, Zone};
