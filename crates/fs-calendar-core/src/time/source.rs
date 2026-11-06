//! The [`ZoneSource`] contract: where time zone rules come from.
//!
//! Pattern: Strategy + Template method — implementors only say how to look a zone up
//! (`lookup`) and which database version they are; the shared id validation and the construction
//! of a [`Zone`] are provided methods, so every source behaves the same for bad input.

use std::path::PathBuf;

use jiff::tz::TimeZone;

use super::zone::{IanaZone, Zone};
use crate::error::{Error, Result};

/// Where the active tz database comes from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TzSource {
    /// A zoneinfo directory of the operating system.
    System(PathBuf),
    /// The copy compiled into the binary (`jiff-tzdb`).
    Bundled,
}

/// A tz database with a version.
pub trait ZoneSource: Send + Sync {
    /// Version of the database, e.g. `2026e` (`<year><letter>`: string order is time order).
    fn version(&self) -> &str;

    /// Which kind of source this is.
    fn source(&self) -> &TzSource;

    /// The raw rules of a zone, if the database knows it. `name` is already validated.
    fn lookup(&self, name: &str) -> Option<TimeZone>;

    /// Look up an IANA zone. Only plain zone ids are accepted (no paths).
    fn zone(&self, name: &str) -> Result<Zone> {
        let ok = !name.is_empty()
            && !name.starts_with('/')
            && !name
                .split('/')
                .any(|p| p.is_empty() || p == "." || p == "..")
            && name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '_' | '-' | '+'));
        if !ok {
            return Err(Error::Time(format!(
                "not a valid IANA time zone id {name:?}"
            )));
        }
        let tz = self
            .lookup(name)
            .ok_or_else(|| Error::Time(format!("unknown IANA time zone {name:?}")))?;
        Ok(Zone::Iana(Box::new(IanaZone::new(name, tz))))
    }
}
