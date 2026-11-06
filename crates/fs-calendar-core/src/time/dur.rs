//! ISO 8601 durations as JSCalendar uses them.
//!
//! Pattern: Value object — nominal days and exact seconds kept apart, so `P1D` stays a calendar day across DST.

use chrono::{DateTime, Duration, NaiveDateTime, Utc};

use super::zone::Zone;
use crate::error::{Error, Result};

/// A JSCalendar duration split into nominal days (weeks included) and exact seconds.
/// `P1D` is a calendar day (23/25 hours across DST), `PT1H` is exactly 3600 s.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Dur {
    /// Nominal days (weeks included): a calendar day, 23 / 25 hours across DST.
    pub days: i64,
    /// Exact seconds.
    pub secs: i64,
}

impl Dur {
    /// Parse an ISO 8601 duration like `P1DT2H30M`.
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
                    if nominal {
                        d.days += v;
                    } else {
                        d.secs += v;
                    }
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
