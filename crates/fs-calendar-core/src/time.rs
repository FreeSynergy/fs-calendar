//! Local date-times, ISO 8601 durations and IANA time zones (chrono-tz).

use chrono::{DateTime, Duration, LocalResult, NaiveDateTime, Offset, TimeZone, Utc};
use chrono_tz::Tz;

use crate::error::{Error, Result};

/// The IANA tz database version compiled in (ADR: record it with each original).
pub const TZDB_VERSION: &str = chrono_tz::IANA_TZDB_VERSION;

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

/// A zone: UTC or an IANA time zone. Floating times are resolved in the zone the caller chooses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Zone {
    Utc,
    Iana(Tz),
}

impl Zone {
    /// `Etc/UTC`, `UTC` and friends map to [`Zone::Utc`]; custom ids (`/...`) are rejected (JSCalendar 2.0: IANA only).
    pub fn parse(name: &str) -> Result<Zone> {
        if name.starts_with('/') {
            return Err(Error::Time(format!("custom time zone {name:?} is not allowed in JSCalendar 2.0")));
        }
        let tz: Tz = name.parse().map_err(|_| Error::Time(format!("unknown IANA time zone {name:?}")))?;
        Ok(Zone::Iana(tz))
    }

    pub fn name(&self) -> &'static str {
        match self {
            Zone::Utc => "Etc/UTC",
            Zone::Iana(t) => t.name(),
        }
    }

    pub fn tz(&self) -> Tz {
        match self {
            Zone::Utc => Tz::UTC,
            Zone::Iana(t) => *t,
        }
    }

    /// Local wall-clock time -> UTC. Ambiguous (clock goes back): the first occurrence (RFC 5545).
    /// Nonexistent (clock jumps forward): interpreted with the offset in force before the gap.
    pub fn to_utc(&self, local: NaiveDateTime) -> DateTime<Utc> {
        let tz = self.tz();
        match tz.from_local_datetime(&local) {
            LocalResult::Single(t) => t.with_timezone(&Utc),
            LocalResult::Ambiguous(first, _) => first.with_timezone(&Utc),
            LocalResult::None => {
                let before = local - Duration::hours(3);
                let off = match tz.from_local_datetime(&before) {
                    LocalResult::Single(t) | LocalResult::Ambiguous(t, _) => t.offset().fix().local_minus_utc(),
                    LocalResult::None => 0,
                };
                (local - Duration::seconds(i64::from(off))).and_utc()
            }
        }
    }

    pub fn to_local(&self, utc: DateTime<Utc>) -> NaiveDateTime {
        utc.with_timezone(&self.tz()).naive_local()
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
        assert!(!TZDB_VERSION.is_empty());
    }
}
