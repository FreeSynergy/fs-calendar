//! Choosing the tz database in use: the newest of system and bundled.
//!
//! Pattern: Factory — builds the `ZoneSource` strategy at first use: the system copy
//! (`$TZDIR`, `/usr/share/zoneinfo`, ...) if its version is known and not older than the bundled
//! one, otherwise the bundled copy. [`tzdb_version`] / [`tzdb_source`] say which; the version is
//! recorded with each imported original.

use std::path::PathBuf;
use std::sync::LazyLock;

use super::bundled::BundledTzdb;
use super::source::{TzSource, ZoneSource};
use super::system::SystemTzdb;

pub(super) static ACTIVE: LazyLock<Box<dyn ZoneSource>> = LazyLock::new(detect);

/// Newest of the usable system databases and the bundled one (ties go to the system copy).
pub fn detect() -> Box<dyn ZoneSource> {
    let mut dirs: Vec<PathBuf> = std::env::var_os("TZDIR")
        .map(PathBuf::from)
        .into_iter()
        .collect();
    dirs.extend(
        [
            "/usr/share/zoneinfo",
            "/usr/lib/zoneinfo",
            "/usr/share/lib/zoneinfo",
            "/etc/zoneinfo",
        ]
        .map(PathBuf::from),
    );
    newest(
        dirs.iter()
            .filter(|d| d.is_dir())
            .find_map(|d| SystemTzdb::from_dir(d)),
    )
}

/// The system database if it is at least as new as the bundled one, else the bundled one.
/// tzdb versions are `<year><letter>`: string order is time order.
pub fn newest(system: Option<SystemTzdb>) -> Box<dyn ZoneSource> {
    let bundled = BundledTzdb::new();
    match system {
        Some(s) if s.version() >= bundled.version() => Box::new(s),
        _ => Box::new(bundled),
    }
}

/// Version of the tz database in use (e.g. `2026e`); recorded with each import.
pub fn tzdb_version() -> &'static str {
    ACTIVE.version()
}

/// Where the tz database in use comes from.
pub fn tzdb_source() -> &'static TzSource {
    ACTIVE.source()
}
