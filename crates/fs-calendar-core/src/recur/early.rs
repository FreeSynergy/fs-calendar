//! The earliest instance of a series, which may lie before its master start.
//!
//! Pattern: Iterator — a `filter_map` / `fold` over the overrides: an override that moves an instance before the
//! series, or an extra instance given there, makes the series start earlier than its master; the
//! envelope and the index must start there, or the store filter hides the series in that week.

use chrono::{DateTime, Utc};
use serde_json::Value;

use super::occurrence::{first_span, zone_of};
use crate::error::Result;
use crate::model::Event;
use crate::time::{Zone, parse_local};

/// The start in UTC of one override, if it moves or adds an instance and reads.
fn override_start(key: &str, patch: &Value, master_zone: &Zone) -> Option<DateTime<Utc>> {
    let patch = patch.as_object()?;
    if patch.get("excluded") == Some(&Value::Bool(true)) {
        return None;
    }
    let local = patch.get("start").and_then(Value::as_str).unwrap_or(key);
    let zone = match patch.get("timeZone").and_then(Value::as_str) {
        Some(name) => Zone::parse(name).ok()?,
        None => master_zone.clone(),
    };
    Some(zone.to_utc(parse_local(local).ok()?))
}

/// The earliest start in UTC among the master and every override that is not excluded. An override
/// that does not read counts for nothing here: it is refused when written, and damaged when read.
pub fn earliest_start(e: &Event, floating: &Zone) -> Result<DateTime<Utc>> {
    let (master, _) = first_span(e, floating)?;
    let master_zone = zone_of(e, floating)?;
    Ok(e.recurrence_overrides
        .iter()
        .filter_map(|(key, patch)| override_start(key, patch, &master_zone))
        .fold(master, DateTime::min))
}
