//! [`Zone`]: UTC or an IANA zone, converting wall-clock time to UTC and back.
//!
//! Pattern: Strategy (client) — zone rules come from the active [`ZoneSource`]; jiff does the
//! DST arithmetic ("compatible" disambiguation), chrono carries the values.

use chrono::{DateTime, Datelike, Duration, NaiveDateTime, Timelike, Utc};
use jiff::tz::TimeZone;

use super::select::ACTIVE;
use super::source::ZoneSource;
use crate::error::{Error, Result};

/// An IANA zone resolved against a tz database.
#[derive(Clone, Debug)]
pub struct IanaZone {
    name: String,
    tz: TimeZone,
}

impl IanaZone {
    pub(super) fn new(name: &str, tz: TimeZone) -> Self {
        Self {
            name: name.to_string(),
            tz,
        }
    }
}

/// A zone: UTC or an IANA time zone. Floating times are resolved in the zone the caller chooses.
#[derive(Clone, Debug)]
pub enum Zone {
    /// Coordinated Universal Time.
    Utc,
    /// An IANA time zone.
    Iana(Box<IanaZone>),
}

impl PartialEq for Zone {
    fn eq(&self, other: &Self) -> bool {
        self.name() == other.name()
    }
}
impl Eq for Zone {}

impl Zone {
    /// Look a zone up in a specific source instead of the active one.
    pub fn parse_in(source: &dyn ZoneSource, name: &str) -> Result<Zone> {
        if name.starts_with('/') {
            return Err(Error::Time(format!(
                "custom time zone {name:?} is not allowed in JSCalendar 2.0"
            )));
        }
        source.zone(name)
    }

    /// IANA id in the active tz database. Custom ids (`/...`) are rejected (JSCalendar 2.0: IANA only).
    /// `Etc/UTC` and `UTC` are real zones with a fixed offset of 0.
    pub fn parse(name: &str) -> Result<Zone> {
        if name.starts_with('/') {
            return Err(Error::Time(format!(
                "custom time zone {name:?} is not allowed in JSCalendar 2.0"
            )));
        }
        ACTIVE.zone(name)
    }

    /// IANA id of the zone (`Etc/UTC` for [`Zone::Utc`]).
    pub fn name(&self) -> &str {
        match self {
            Zone::Utc => "Etc/UTC",
            Zone::Iana(z) => &z.name,
        }
    }

    /// Local wall-clock time -> UTC. Ambiguous (clock goes back): the first occurrence (RFC 5545).
    /// Nonexistent (clock jumps forward): interpreted with the offset in force before the gap.
    /// (Both are jiff's "compatible" disambiguation.) Out-of-range years are taken as UTC.
    // The components come from a valid `NaiveDateTime` (month 1-12, day 1-31, ...); only the year
    // can exceed jiff's range, and then `civil.ok()` fails and the time is taken as UTC.
    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
    pub fn to_utc(&self, local: NaiveDateTime) -> DateTime<Utc> {
        let Zone::Iana(z) = self else {
            return local.and_utc();
        };
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

    /// UTC -> local wall-clock time in this zone.
    pub fn to_local(&self, utc: DateTime<Utc>) -> NaiveDateTime {
        let Zone::Iana(z) = self else {
            return utc.naive_utc();
        };
        match jiff::Timestamp::from_second(utc.timestamp()) {
            Ok(ts) => utc.naive_utc() + Duration::seconds(i64::from(z.tz.to_offset(ts).seconds())),
            Err(_) => utc.naive_utc(),
        }
    }
}
