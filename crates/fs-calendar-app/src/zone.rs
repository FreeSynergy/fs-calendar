//! Which time zone is shown.
//!
//! Pattern: Strategy by fallback — what was asked for, else `TZ`, else the system's link, else UTC.
//! Zones come from the core's zone source, so a wrong name is refused the same way everywhere.

use std::path::Path;

use fs_calendar_core::Zone;

/// The zone named `asked`, or the system's.
///
/// # Errors
/// A message for the person if `asked` names no zone.
pub fn shown_zone(asked: Option<&str>) -> Result<Zone, String> {
    if let Some(name) = asked {
        return Zone::parse(name).map_err(|e| format!("--zone {name}: {e}"));
    }
    let named = std::env::var("TZ")
        .ok()
        .filter(|name| !name.is_empty())
        .or_else(|| zone_of_link(Path::new("/etc/localtime")));
    Ok(named
        .and_then(|name| Zone::parse(name.trim_start_matches(':')).ok())
        .unwrap_or(Zone::Utc))
}

fn zone_of_link(link: &Path) -> Option<String> {
    let target = std::fs::read_link(link).ok()?;
    let target = target.to_string_lossy();
    let (_, name) = target.split_once("zoneinfo/")?;
    Some(name.to_string())
}
