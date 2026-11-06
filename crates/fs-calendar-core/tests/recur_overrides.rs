//! Recurrence overrides, bad rules, floating zones and the serde / property tests.

mod recur_common;

use chrono::Duration;
use fs_calendar_core::recur::{expand, series_end};
use fs_calendar_core::time::{Zone, fmt_utc};
use fs_calendar_core::{Event, RecurrenceRule};
use proptest::prelude::*;
use recur_common::{WIDE, series, utc};
use serde_json::json;

#[test]
fn overrides_exclude_patch_and_add() {
    let mut e = series(
        "2026-11-03T10:00:00",
        "Etc/UTC",
        json!({"frequency":"weekly","count":4}),
    );
    e.recurrence_overrides
        .insert("2026-11-10T10:00:00".into(), json!({"excluded": true}));
    e.recurrence_overrides.insert("2026-11-17T10:00:00".into(), json!({"title": "Moved", "start": "2026-11-17T14:00:00", "participants/p9": {"name": "Guest"}}));
    e.recurrence_overrides
        .insert("2026-11-20T09:00:00".into(), json!({"title": "Extra"}));
    let occ = expand(&e, utc(WIDE.0), utc(WIDE.1), &Zone::Utc, 100).unwrap();
    let got: Vec<_> = occ
        .iter()
        .map(|o| (fmt_utc(o.start_utc), o.event.display_title(), o.overridden))
        .collect();
    assert_eq!(
        got,
        [
            ("2026-11-03T10:00:00Z".into(), "Series".into(), false),
            ("2026-11-17T14:00:00Z".into(), "Moved".into(), true),
            ("2026-11-20T09:00:00Z".into(), "Extra".into(), true),
            ("2026-11-24T10:00:00Z".into(), "Series".into(), false),
        ]
    );
    assert_eq!(occ[1].recurrence_id, "2026-11-17T10:00:00");
    assert_eq!(
        occ[1].event.participants["p9"].name.as_deref(),
        Some("Guest")
    );
    assert!(occ[1].event.recurrence_rule.is_none());
    // the extra instance counts for the end of the series
    assert_eq!(
        series_end(&e, &Zone::Utc).unwrap().map(fmt_utc).as_deref(),
        Some("2026-11-24T11:00:00Z")
    );
}

#[test]
fn bad_rules_are_errors() {
    let e = series(
        "2026-11-03T10:00:00",
        "Etc/UTC",
        json!({"frequency":"weekly","rscale":"hebrew"}),
    );
    assert!(expand(&e, utc(WIDE.0), utc(WIDE.1), &Zone::Utc, 10).is_err());
    let e = series(
        "2026-11-03T10:00:00",
        "Etc/UTC",
        json!({"frequency":"fortnightly"}),
    );
    assert!(expand(&e, utc(WIDE.0), utc(WIDE.1), &Zone::Utc, 10).is_err());
}

#[test]
fn floating_events_use_the_callers_zone() {
    let mut e = Event::new("Floating", "2026-11-24T10:00:00", "Etc/UTC", "PT1H");
    e.time_zone = None;
    let berlin = Zone::parse("Europe/Berlin").unwrap();
    let o = expand(&e, utc(WIDE.0), utc(WIDE.1), &berlin, 10).unwrap();
    assert_eq!(fmt_utc(o[0].start_utc), "2026-11-24T09:00:00Z");
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// DAILY/WEEKLY with COUNT: exactly COUNT instances, strictly increasing, spaced by interval in local days.
    #[test]
    fn count_interval_spacing(weekly in any::<bool>(), interval in 1u32..6, count in 1u32..40, day in 1u32..28, month in 1u32..13, hour in 0u32..24, tz in prop::sample::select(vec!["Etc/UTC", "Europe/Berlin", "America/New_York", "Asia/Kolkata"])) {
        let start = format!("2026-{month:02}-{day:02}T{hour:02}:00:00");
        let freq = if weekly { "weekly" } else { "daily" };
        let e = series(&start, tz, json!({"frequency": freq, "interval": interval, "count": count}));
        let occ = expand(&e, utc(WIDE.0), utc(WIDE.1), &Zone::Utc, 10_000).unwrap();
        prop_assert_eq!(occ.len() as u32, count);
        let step = i64::from(interval) * if weekly { 7 } else { 1 };
        let zone = Zone::parse(tz).unwrap();
        let first_local = zone.to_local(occ[0].start_utc);
        for (i, o) in occ.iter().enumerate() {
            let local = zone.to_local(o.start_utc);
            // wall clock stays, calendar day advances by `step` (DST gaps may shift a nonexistent time; hours stay except in gaps)
            prop_assert_eq!(local.date(), first_local.date() + Duration::days(step * i as i64));
            if i > 0 { prop_assert!(o.start_utc > occ[i - 1].start_utc); }
            prop_assert_eq!(o.end_utc - o.start_utc, Duration::hours(1));
        }
        prop_assert_eq!(series_end(&e, &Zone::Utc).unwrap(), Some(occ.last().unwrap().end_utc));
    }

    /// Expanding a sub-window gives exactly the full expansion restricted to the window.
    #[test]
    fn window_consistency(freq in prop::sample::select(vec!["daily", "weekly", "monthly", "yearly"]), interval in 1u32..4, off1 in 0i64..400, len in 1i64..200) {
        let e = series("2026-01-15T09:30:00", "Europe/Berlin", json!({"frequency": freq, "interval": interval, "count": 60}));
        let all = expand(&e, utc(WIDE.0), utc(WIDE.1), &Zone::Utc, 10_000).unwrap();
        let from = utc("2026-01-01T00:00:00Z") + Duration::days(off1);
        let to = from + Duration::days(len);
        let win = expand(&e, from, to, &Zone::Utc, 10_000).unwrap();
        let expect: Vec<_> = all.iter().filter(|o| o.start_utc < to && o.end_utc > from).map(|o| o.start_utc).collect();
        prop_assert_eq!(win.iter().map(|o| o.start_utc).collect::<Vec<_>>(), expect);
    }

    /// UNTIL is inclusive and nothing starts after it.
    #[test]
    fn until_bounds(interval in 1u32..5, days in 0i64..120) {
        let until = (utc("2026-03-01T10:00:00Z") + Duration::days(days)).format("%Y-%m-%dT%H:%M:%S").to_string();
        let e = series("2026-03-01T10:00:00", "Etc/UTC", json!({"frequency":"daily","interval": interval, "until": until}));
        let occ = expand(&e, utc(WIDE.0), utc(WIDE.1), &Zone::Utc, 10_000).unwrap();
        prop_assert_eq!(occ.len() as i64, days / i64::from(interval) + 1);
        prop_assert!(occ.iter().all(|o| fmt_utc(o.start_utc).trim_end_matches('Z') <= until.as_str()));
    }
}

#[test]
fn rule_roundtrip_serde() {
    let v = json!({"@type":"RecurrenceRule","frequency":"monthly","interval":2,"byDay":[{"@type":"NDay","day":"fr","nthOfPeriod":-1}],"count":5,"example.com:x":1});
    let r: RecurrenceRule = serde_json::from_value(v.clone()).unwrap();
    assert_eq!(serde_json::to_value(&r).unwrap(), v);
}
