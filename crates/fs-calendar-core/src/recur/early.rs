//! The earliest instance of a series, which may lie before its master start.
//!
//! Pattern: Iterator-style scan over the overrides — an override that moves an instance before the
//! series, or an extra instance given there, makes the series start earlier than its master; the
//! envelope and the index must start there, or the store filter hides the series in that week.

use chrono::{DateTime, Utc};
use serde_json::Value;

use super::occurrence::{first_span, zone_of};
use crate::error::Result;
use crate::model::Event;
use crate::time::{Zone, parse_local};

/// The earliest start in UTC among the master and every override that is not excluded. An override
/// that does not read counts for nothing here: it is refused when written, and damaged when read.
pub fn earliest_start(e: &Event, floating: &Zone) -> Result<DateTime<Utc>> {
    let (mut best, _) = first_span(e, floating)?;
    let master_zone = zone_of(e, floating)?;
    for (key, patch) in &e.recurrence_overrides {
        let Some(patch) = patch.as_object() else {
            continue;
        };
        if patch.get("excluded") == Some(&Value::Bool(true)) {
            continue;
        }
        let local = patch.get("start").and_then(Value::as_str).unwrap_or(key);
        let zone = match patch.get("timeZone").and_then(Value::as_str) {
            Some(name) => match Zone::parse(name) {
                Ok(zone) => zone,
                Err(_) => continue,
            },
            None => master_zone.clone(),
        };
        if let Ok(local) = parse_local(local) {
            best = best.min(zone.to_utc(local));
        }
    }
    Ok(best)
}
