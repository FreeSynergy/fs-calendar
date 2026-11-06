//! Helpers shared by the recurrence tests.

#![allow(dead_code)] // each test file uses a different subset

use chrono::{DateTime, Utc};
use fs_calendar_core::Event;
use fs_calendar_core::recur::expand;
use fs_calendar_core::time::{Zone, fmt_utc};

pub fn utc(s: &str) -> DateTime<Utc> {
    fs_calendar_core::utc(s).unwrap()
}

pub fn series(start: &str, tz: &str, rule: serde_json::Value) -> Event {
    let mut e = Event::new("Series", start, tz, "PT1H");
    e.recurrence_rule = Some(serde_json::from_value(rule).unwrap());
    e
}

pub fn starts(e: &Event, from: &str, to: &str) -> Vec<String> {
    expand(e, utc(from), utc(to), &Zone::Utc, 10_000)
        .unwrap()
        .iter()
        .map(|o| fmt_utc(o.start_utc))
        .collect()
}

/// A window wide enough to see a whole series.
pub const WIDE: (&str, &str) = ("2020-01-01T00:00:00Z", "2040-01-01T00:00:00Z");
