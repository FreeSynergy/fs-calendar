//! A zoneinfo directory of the operating system as a tz database.
//!
//! Pattern: Strategy (concrete) — follows the OS's tz updates; only usable if its version can be read.

use std::path::Path;

use jiff::tz::{TimeZone, TimeZoneDatabase};

use super::source::{TzSource, ZoneSource};

/// A system zoneinfo directory with a known version.
pub struct SystemTzdb {
    db: TimeZoneDatabase,
    version: String,
    source: TzSource,
}

impl SystemTzdb {
    /// A zoneinfo directory; `None` if it has no readable version (we could not record it).
    pub fn from_dir(dir: &Path) -> Option<Self> {
        let version = dir_version(dir)?;
        let db = TimeZoneDatabase::from_dir(dir).ok()?;
        Some(Self {
            db,
            version,
            source: TzSource::System(dir.to_path_buf()),
        })
    }
}

impl ZoneSource for SystemTzdb {
    fn version(&self) -> &str {
        &self.version
    }

    fn source(&self) -> &TzSource {
        &self.source
    }

    fn lookup(&self, name: &str) -> Option<TimeZone> {
        self.db.get(name).ok()
    }
}

/// `# version 2026e` (first line of `tzdata.zi`) or the content of a `+VERSION` file.
fn dir_version(dir: &Path) -> Option<String> {
    let clean = |s: &str| s.trim().to_string();
    if let Ok(zi) = std::fs::read_to_string(dir.join("tzdata.zi")) {
        if let Some(v) = zi.lines().next().and_then(|l| l.strip_prefix("# version ")) {
            return Some(clean(v)).filter(|v| !v.is_empty());
        }
    }
    std::fs::read_to_string(dir.join("+VERSION"))
        .ok()
        .map(|s| clean(&s))
        .filter(|v| !v.is_empty())
}
