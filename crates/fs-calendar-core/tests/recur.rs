//! Recurrence expansion: rule families, DST behaviour and query windows.

mod recur_common;

use fs_calendar_core::Event;
use fs_calendar_core::recur::series_end;
use fs_calendar_core::time::{Zone, fmt_utc};
use recur_common::{WIDE, series, starts};
use serde_json::json;

#[test]
fn no_rule_is_single_instance() {
    let e = Event::new("One", "2026-11-24T10:00:00", "Europe/Berlin", "PT1H");
    assert_eq!(starts(&e, WIDE.0, WIDE.1), ["2026-11-24T09:00:00Z"]);
    // half-open: an event ending exactly at `from` does not overlap; starting exactly at `to` does not either
    assert_eq!(
        starts(&e, "2026-11-24T10:00:00Z", WIDE.1),
        [] as [std::string::String; 0]
    );
    assert_eq!(
        starts(&e, WIDE.0, "2026-11-24T09:00:00Z"),
        [] as [std::string::String; 0]
    );
    assert_eq!(starts(&e, "2026-11-24T09:59:59Z", WIDE.1).len(), 1);
}

#[test]
fn daily_interval_count() {
    let e = series(
        "2026-11-01T08:00:00",
        "Etc/UTC",
        json!({"frequency":"daily","interval":3,"count":4}),
    );
    assert_eq!(
        starts(&e, WIDE.0, WIDE.1),
        [
            "2026-11-01T08:00:00Z",
            "2026-11-04T08:00:00Z",
            "2026-11-07T08:00:00Z",
            "2026-11-10T08:00:00Z"
        ]
    );
    assert_eq!(
        series_end(&e, &Zone::Utc).unwrap().map(fmt_utc).as_deref(),
        Some("2026-11-10T09:00:00Z")
    );
}

#[test]
fn weekly_byday_until() {
    // Tue+Thu, until Thu 2026-11-12 (local, inclusive)
    let e = series(
        "2026-11-03T10:00:00",
        "Europe/Berlin",
        json!({"frequency":"weekly","byDay":[{"day":"tu"},{"day":"th"}],"until":"2026-11-12T10:00:00"}),
    );
    assert_eq!(
        starts(&e, WIDE.0, WIDE.1),
        [
            "2026-11-03T09:00:00Z",
            "2026-11-05T09:00:00Z",
            "2026-11-10T09:00:00Z",
            "2026-11-12T09:00:00Z"
        ]
    );
}

#[test]
fn monthly_nth_weekday_and_monthday() {
    // second Tuesday of each month
    let e = series(
        "2026-01-13T09:00:00",
        "Etc/UTC",
        json!({"frequency":"monthly","byDay":[{"day":"tu","nthOfPeriod":2}],"count":3}),
    );
    assert_eq!(
        starts(&e, WIDE.0, WIDE.1),
        [
            "2026-01-13T09:00:00Z",
            "2026-02-10T09:00:00Z",
            "2026-03-10T09:00:00Z"
        ]
    );
    // last day of the month
    let e = series(
        "2026-01-31T09:00:00",
        "Etc/UTC",
        json!({"frequency":"monthly","byMonthDay":[-1],"count":3}),
    );
    assert_eq!(
        starts(&e, WIDE.0, WIDE.1),
        [
            "2026-01-31T09:00:00Z",
            "2026-02-28T09:00:00Z",
            "2026-03-31T09:00:00Z"
        ]
    );
}

#[test]
fn yearly_and_leap_day() {
    let e = series(
        "2026-05-01T12:00:00",
        "Etc/UTC",
        json!({"frequency":"yearly","count":3}),
    );
    assert_eq!(
        starts(&e, WIDE.0, WIDE.1),
        [
            "2026-05-01T12:00:00Z",
            "2027-05-01T12:00:00Z",
            "2028-05-01T12:00:00Z"
        ]
    );
    let e = series(
        "2024-02-29T12:00:00",
        "Etc/UTC",
        json!({"frequency":"yearly","count":3}),
    );
    assert_eq!(
        starts(&e, WIDE.0, WIDE.1),
        [
            "2024-02-29T12:00:00Z",
            "2028-02-29T12:00:00Z",
            "2032-02-29T12:00:00Z"
        ]
    );
}

#[test]
fn wall_clock_time_is_kept_across_dst() {
    let e = series(
        "2026-10-20T10:00:00",
        "Europe/Berlin",
        json!({"frequency":"weekly","count":3}),
    );
    // 10:00 local = 08:00Z (CEST) before 25 Oct, 09:00Z (CET) after
    assert_eq!(
        starts(&e, WIDE.0, WIDE.1),
        [
            "2026-10-20T08:00:00Z",
            "2026-10-27T09:00:00Z",
            "2026-11-03T09:00:00Z"
        ]
    );
}

#[test]
fn query_window_clips_and_long_instances_overlap() {
    let mut e = series(
        "2026-11-01T22:00:00",
        "Etc/UTC",
        json!({"frequency":"daily"}),
    );
    e.duration = Some("PT4H".into()); // 22:00 -> 02:00 next day
    // window starts at 01:00 on the 5th: the instance of the 4th is still running
    let got = starts(&e, "2026-11-05T01:00:00Z", "2026-11-06T23:00:00Z");
    assert_eq!(
        got,
        [
            "2026-11-04T22:00:00Z",
            "2026-11-05T22:00:00Z",
            "2026-11-06T22:00:00Z"
        ]
    );
    // unbounded series have no end
    assert_eq!(series_end(&e, &Zone::Utc).unwrap(), None);
}
