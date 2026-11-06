//! The tz database compiled into the binary.
//!
//! Pattern: Strategy (concrete) — always available, version fixed at build time.

use jiff::tz::{TimeZone, TimeZoneDatabase};

use super::source::{TzSource, ZoneSource};

/// The copy bundled in `jiff-tzdb`.
pub struct BundledTzdb {
    db: TimeZoneDatabase,
}

impl BundledTzdb {
    /// The bundled database.
    pub fn new() -> Self {
        Self {
            db: TimeZoneDatabase::bundled(),
        }
    }
}

impl Default for BundledTzdb {
    fn default() -> Self {
        Self::new()
    }
}

impl ZoneSource for BundledTzdb {
    fn version(&self) -> &str {
        jiff_tzdb::VERSION.unwrap_or("0000")
    }

    fn source(&self) -> &TzSource {
        &TzSource::Bundled
    }

    fn lookup(&self, name: &str) -> Option<TimeZone> {
        self.db.get(name).ok()
    }
}
