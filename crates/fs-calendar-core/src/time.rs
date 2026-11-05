//! Local date-times, ISO 8601 durations and IANA time zones (jiff + tz database, ADR 0026).
//!
//! **Which tz database.** The newest available one: the system copy (`$TZDIR`, `/usr/share/zoneinfo`,
//! ...) if its version is known (`tzdata.zi` first line / `+VERSION`) and not older than the copy
//! bundled in `jiff-tzdb`; otherwise the bundled copy. [`tzdb_version`] / [`tzdb_source`] say which,
//! and the version is recorded with each imported original.

use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use chrono::{DateTime, Duration, NaiveDateTime, Timelike, Utc};
use jiff::tz::{TimeZone, TimeZoneDatabase};

use crate::error::{Error, Result};

/// Where the active tz database comes from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TzSource {
    /// A zoneinfo directory of the operating system.
    System(PathBuf),
    /// The copy compiled into the binary (`jiff-tzdb`).
    Bundled,
}

/// A tz database with its version.
pub struct TzDb {
    db: TimeZoneDatabase,
    version: String,
    source: TzSource,
}

impl TzDb {
    /// The bundled database.
    pub fn bundled() -> TzDb {
        TzDb {
            db: TimeZoneDatabase::bundled(),
            version: bundled_version().to_string(),
            source: TzSource::Bundled,
        }
    }

    /// A zoneinfo directory; `None` if it has no readable version (we could not record it).
    pub fn from_dir(dir: &Path) -> Option<TzDb> {
        let version = dir_version(dir)?;
        let db = TimeZoneDatabase::from_dir(dir).ok()?;
        Some(TzDb { db, version, source: TzSource::System(dir.to_path_buf()) })
    }

    /// Newest of the usable system databases and the bundled one (ties go to the system copy).
    pub fn detect() -> TzDb {
        let mut dirs: Vec<PathBuf> = std::env::var_os("TZDIR").map(PathBuf::from).into_iter().collect();
        dirs.extend(["/usr/share/zoneinfo", "/usr/lib/zoneinfo", "/usr/share/lib/zoneinfo", "/etc/zoneinfo"].map(PathBuf::from));
        let system = dirs.iter().filter(|d| d.is_dir()).find_map(|d| TzDb::from_dir(d));
        match system {
            // tzdb versions are `<year><letter>`: string order is time order.
            Some(s) if s.version.as_str() >= bundled_version() => s,
            _ => TzDb::bundled(),
        }
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn source(&self) -> &TzSource {
        &self.source
    }

    /// Look up an IANA zone. Only plain zone ids are accepted (no paths).
    pub fn zone(&self, name: &str) -> Result<Zone> {
        let ok = !name.is_empty()
            && !name.starts_with('/')
            && !name.split('/').any(|p| p.is_empty() || p == "." || p == "..")
            && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '_' | '-' | '+'));
        if !ok {
            return Err(Error::Time(format!("not a valid IANA time zone id {name:?}")));
        }
        let tz = self.db.get(name).map_err(|_| Error::Time(format!("unknown IANA time zone {name:?}")))?;
        Ok(Zone::Iana(Box::new(IanaZone { name: name.to_string(), tz })))
    }
}

fn bundled_version() -> &'static str {
    jiff_tzdb::VERSION.unwrap_or("0000")
}

/// `# version 2026e` (first line of `tzdata.zi`) or the content of a `+VERSION` file.
fn dir_version(dir: &Path) -> Option<String> {
    let clean = |s: &str| s.trim().to_string();
    if let Ok(zi) = std::fs::read_to_string(dir.join("tzdata.zi")) {
        if let Some(v) = zi.lines().next().and_then(|l| l.strip_prefix("# version ")) {
            return Some(clean(v)).filter(|v| !v.is_empty());
        }
    }
    std::fs::read_to_string(dir.join("+VERSION")).ok().map(|s| clean(&s)).filter(|v| !v.is_empty())
}

static TZDB: LazyLock<TzDb> = LazyLock::new(TzDb::detect);

/// Version of the tz database in use (e.g. `2026e`); recorded with each import.
pub fn tzdb_version() -> &'static str {
    TZDB.version()
}

/// Where the tz database in use comes from.
pub fn tzdb_source() -> &'static TzSource {
    TZDB.source()
}

/// Format of UTC index values: lexicographic order == time order.
pub const UTC_FORMAT: &str = "%Y-%m-%dT%H:%M:%SZ";
/// Far-future marker for series without end.
pub const FOREVER: &str = "9999-12-31T23:59:59Z";

pub fn fmt_utc(t: DateTime<Utc>) -> String {
    t.format(UTC_FORMAT).to_string()
}

/// JSCalendar `LocalDateTime` (`2026-11-24T10:00:00`, no zone, no fraction in 2.0).
pub fn parse_local(s: &str) -> Result<NaiveDateTime> {
    if s.len() != 19 {
        return Err(Error::Time(format!("not a LocalDateTime: {s:?}")));
    }
    NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S").map_err(|_| Error::Time(format!("not a LocalDateTime: {s:?}")))
}

pub fn fmt_local(t: NaiveDateTime) -> String {
    t.format("%Y-%m-%dT%H:%M:%S").to_string()
}

/// `UTCDateTime`: `2026-11-05T09:00:00Z`.
pub fn parse_utc(s: &str) -> Result<DateTime<Utc>> {
    s.strip_suffix('Z')
        .filter(|x| x.len() == 19)
        .and_then(|x| NaiveDateTime::parse_from_str(x, "%Y-%m-%dT%H:%M:%S").ok())
        .map(|n| n.and_utc())
        .ok_or_else(|| Error::Time(format!("not a UTCDateTime: {s:?}")))
}

/// An IANA zone resolved against a tz database.
#[derive(Clone, Debug)]
pub struct IanaZone {
    name: String,
    tz: TimeZone,
}

/// A zone: UTC or an IANA time zone. Floating times are resolved in the zone the caller chooses.
#[derive(Clone, Debug)]
pub enum Zone {
    Utc,
    Iana(Box<IanaZone>),
}

impl PartialEq for Zone {
    fn eq(&self, other: &Self) -> bool {
        self.name() == other.name()
    }
}
impl Eq for Zone {}

impl Zone {
    /// IANA id in the active tz database. Custom ids (`/...`) are rejected (JSCalendar 2.0: IANA only).
    /// `Etc/UTC` and `UTC` are real zones with a fixed offset of 0.
    pub fn parse(name: &str) -> Result<Zone> {
        if name.starts_with('/') {
            return Err(Error::Time(format!("custom time zone {name:?} is not allowed in JSCalendar 2.0")));
        }
        TZDB.zone(name)
    }

    pub fn name(&self) -> &str {
        match self {
            Zone::Utc => "Etc/UTC",
            Zone::Iana(z) => &z.name,
        }
    }

    /// Local wall-clock time -> UTC. Ambiguous (clock goes back): the first occurrence (RFC 5545).
    /// Nonexistent (clock jumps forward): interpreted with the offset in force before the gap.
    /// (Both are jiff's "compatible" disambiguation.) Out-of-range years are taken as UTC.
    pub fn to_utc(&self, local: NaiveDateTime) -> DateTime<Utc> {
        let Zone::Iana(z) = self else { return local.and_utc() };
        use chrono::Datelike;
        let civil = jiff::civil::DateTime::new(
            local.year() as i16,
            local.month() as i8,
            local.day() as i8,
            local.hour() as i8,
            local.minute() as i8,
            local.second() as i8,
            0,
        );
        civil
            .ok()
            .and_then(|c| z.tz.to_ambiguous_timestamp(c).compatible().ok())
            .and_then(|ts| DateTime::from_timestamp(ts.as_second(), 0))
            .unwrap_or_else(|| local.and_utc())
    }

    pub fn to_local(&self, utc: DateTime<Utc>) -> NaiveDateTime {
        let Zone::Iana(z) = self else { return utc.naive_utc() };
        match jiff::Timestamp::from_second(utc.timestamp()) {
            Ok(ts) => utc.naive_utc() + Duration::seconds(i64::from(z.tz.to_offset(ts).seconds())),
            Err(_) => utc.naive_utc(),
        }
    }
}

/// A JSCalendar duration split into nominal days (weeks included) and exact seconds.
/// `P1D` is a calendar day (23/25 hours across DST), `PT1H` is exactly 3600 s.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Dur {
    pub days: i64,
    pub secs: i64,
}

impl Dur {
    pub fn parse(s: &str) -> Result<Dur> {
        let bad = || Error::Time(format!("not a duration: {s:?}"));
        let rest = s.strip_prefix('P').ok_or_else(bad)?;
        let (date, time) = match rest.split_once('T') {
            Some((d, t)) => (d, Some(t)),
            None => (rest, None),
        };
        let mut d = Dur::default();
        let mut any = false;
        let mut take = |part: &str, units: &[(char, i64, bool)]| -> Result<()> {
            let mut num = String::new();
            for c in part.chars() {
                if c.is_ascii_digit() {
                    num.push(c);
                } else {
                    let &(_, mult, nominal) = units.iter().find(|u| u.0 == c).ok_or_else(bad)?;
                    let n: i64 = num.parse().map_err(|_| bad())?;
                    num.clear();
                    let v = n.checked_mul(mult).ok_or_else(bad)?;
                    if nominal { d.days += v } else { d.secs += v }
                    any = true;
                }
            }
            if num.is_empty() { Ok(()) } else { Err(bad()) }
        };
        take(date, &[('W', 7, true), ('D', 1, true)])?;
        if let Some(t) = time {
            if t.is_empty() {
                return Err(bad());
            }
            take(t, &[('H', 3600, false), ('M', 60, false), ('S', 1, false)])?;
        }
        if any { Ok(d) } else { Err(bad()) }
    }

    /// End of `start` (local, in `zone`) after this duration, as UTC.
    pub fn end_utc(&self, zone: &Zone, start_local: NaiveDateTime) -> DateTime<Utc> {
        let shifted = start_local + Duration::days(self.days);
        zone.to_utc(shifted) + Duration::seconds(self.secs)
    }

    /// Upper bound of the length in seconds (days counted as 25 h).
    pub fn max_secs(&self) -> i64 {
        self.days * 25 * 3600 + self.secs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations() {
        assert_eq!(Dur::parse("PT1H").unwrap(), Dur { days: 0, secs: 3600 });
        assert_eq!(Dur::parse("P1DT2H30M").unwrap(), Dur { days: 1, secs: 9000 });
        assert_eq!(Dur::parse("P2W").unwrap(), Dur { days: 14, secs: 0 });
        for bad in ["", "P", "PT", "1H", "PT1", "P1H", "PTH", "P-1D"] {
            assert!(Dur::parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn zones_and_dst() {
        let berlin = Zone::parse("Europe/Berlin").unwrap();
        let l = parse_local("2026-11-24T10:00:00").unwrap();
        assert_eq!(fmt_utc(berlin.to_utc(l)), "2026-11-24T09:00:00Z");
        let l = parse_local("2026-07-01T10:00:00").unwrap();
        assert_eq!(fmt_utc(berlin.to_utc(l)), "2026-07-01T08:00:00Z");
        // gap: 2026-03-29 02:30 does not exist in Berlin; offset before the gap is +1 -> 01:30Z
        let gap = parse_local("2026-03-29T02:30:00").unwrap();
        assert_eq!(fmt_utc(berlin.to_utc(gap)), "2026-03-29T01:30:00Z");
        // overlap: 2026-10-25 02:30 happens twice; first is CEST (+2) -> 00:30Z
        let dup = parse_local("2026-10-25T02:30:00").unwrap();
        assert_eq!(fmt_utc(berlin.to_utc(dup)), "2026-10-25T00:30:00Z");
        // a nominal day across the DST change is 25 h
        let start = parse_local("2026-10-24T12:00:00").unwrap();
        let end = Dur::parse("P1D").unwrap().end_utc(&berlin, start);
        assert_eq!(end - berlin.to_utc(start), Duration::hours(24 + 1));
    }

    #[test]
    fn rejects_non_iana() {
        assert!(Zone::parse("/Custom/Zone").is_err());
        assert!(Zone::parse("Mars/Olympus").is_err());
        assert!(parse_local("2026-11-24T10:00:00Z").is_err());
        assert!(parse_local("2026-11-24T10:00:00.5").is_err());
        assert!(!tzdb_version().is_empty());
    }
}
