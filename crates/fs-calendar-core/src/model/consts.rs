//! Shared aliases and constants of the JSCalendar model.
//!
//! Pattern: Newtype (alias form) — names the JSCalendar `Id[T]` map and the verbatim-extras map once.

use std::collections::BTreeMap;

use serde_json::Value;

/// `Id[T]` map of JSCalendar.
pub type Map<T> = BTreeMap<String, T>;
/// Unknown / vendor properties, preserved verbatim.
pub type Extra = BTreeMap<String, Value>;

/// The JSCalendar version this crate writes.
pub const VERSION: &str = "2.0";
/// Vendor prefix (ADR 0028).
pub const VENDOR: &str = "freesynergy.net:";
/// Where obsolete 1.0 properties go when upgrading (see [`crate::upgrade`]).
pub const LEGACY_KEY: &str = "freesynergy.net:legacy";

pub(super) fn is_empty_map<T>(m: &Map<T>) -> bool {
    m.is_empty()
}
