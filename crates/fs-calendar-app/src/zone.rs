//! Which time zone is shown.
//!
//! Pattern: Strategy by fallback — what was asked for, else `TZ`, else the system's link, else UTC.
//! Zones come from the core's zone source, so a wrong name is refused the same way everywhere. A `TZ`
//! that names nothing means UTC, as it does for other programs, but it is said.

use std::path::Path;

use fs_calendar_core::Zone;

/// The zone named `asked`, or the system's, and what the person should be told about it.
///
/// # Errors
/// A message for the person if `asked` names no zone.
pub fn shown_zone(asked: Option<&str>) -> Result<(Zone, Option<String>), String> {
    let tz = std::env::var("TZ").ok();
    zone_from(
        asked,
        tz.as_deref(),
        zone_of_link(Path::new("/etc/localtime")),
    )
}

/// [`shown_zone`] with the environment given.
///
/// # Errors
/// A message for the person if `asked` names no zone.
pub fn zone_from(
    asked: Option<&str>,
    tz: Option<&str>,
    link: Option<String>,
) -> Result<(Zone, Option<String>), String> {
    if let Some(name) = asked {
        let zone = Zone::parse(name).map_err(|e| format!("--zone {name}: {e}"))?;
        return Ok((zone, None));
    }
    if let Some(name) = tz
        .map(|n| n.trim_start_matches(':'))
        .filter(|n| !n.is_empty())
    {
        return Ok(match Zone::parse(name) {
            Ok(zone) => (zone, None),
            Err(_) => (
                Zone::Utc,
                Some(format!(
                    "TZ names no time zone (\"{name}\"), so times are shown in UTC"
                )),
            ),
        });
    }
    if let Some(zone) = link.and_then(|name| Zone::parse(&name).ok()) {
        return Ok((zone, None));
    }
    Ok((
        Zone::Utc,
        Some("The system names no time zone, so times are shown in UTC".to_string()),
    ))
}

fn zone_of_link(link: &Path) -> Option<String> {
    let target = std::fs::read_link(link).ok()?;
    let target = target.to_string_lossy();
    let (_, name) = target.split_once("zoneinfo/")?;
    Some(name.to_string())
}
