//! Bounds: a duration beyond range, a window far from the series start, too many instances, size.

mod common;

use std::time::{Duration, Instant};

use common::{repo, t};
use fs_calendar_core::limits::{MAX_EVENT_BYTES, MAX_INSTANCES_PER_EVENT};
use fs_calendar_core::*;
use serde_json::json;

fn daily(title: &str, start: &str) -> Event {
    let mut e = Event::new(title, start, "Etc/UTC", "PT1H");
    e.recurrence_rule = Some(serde_json::from_value(json!({"frequency":"daily"})).unwrap());
    e
}

#[tokio::test]
async fn a_duration_beyond_range_in_a_file_does_not_panic() {
    let r = repo().await;
    let ics = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:x\r\nBEGIN:VEVENT\r\nUID:d1\r\nDTSTAMP:20261101T000000Z\r\nDTSTART:20261201T090000Z\r\nDURATION:P999999999D\r\nSUMMARY:Long\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nUID:d2\r\nDTSTAMP:20261101T000000Z\r\nDTSTART:20261201T090000Z\r\nSUMMARY:Fine\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
    let report = r.import_text("text/calendar", ics).await.unwrap();
    assert_eq!(report.created.len() + report.failed.len(), 2, "{report:?}");
    assert!(
        report.failed.iter().any(|f| f.starts_with("d1")),
        "{report:?}"
    );
}

#[tokio::test]
async fn a_window_far_from_the_series_start_is_walked_from_its_own_start_where_the_rule_allows() {
    let r = repo().await;
    r.create(daily("Daily", "2020-01-01T09:00:00"))
        .await
        .unwrap();
    let mut other = Event::new("Single", "8999-06-01T09:00:00", "Etc/UTC", "PT1H");
    other.uid = "single".into();
    r.create(other).await.unwrap();
    let began = Instant::now();
    let window = r
        .window(t("8999-06-01T00:00:00Z"), t("8999-06-08T00:00:00Z"))
        .await
        .unwrap();
    assert!(
        began.elapsed() < Duration::from_secs(1),
        "{:?}",
        began.elapsed()
    );
    assert_eq!(
        window.instances.len(),
        8,
        "seven days of the series and the single event"
    );
    assert!(window.damaged.is_empty(), "{:?}", window.damaged);
}

#[tokio::test]
async fn an_event_with_more_instances_than_its_limit_is_named_alone() {
    let r = repo().await;
    r.create(daily("Daily", "2026-01-01T09:00:00"))
        .await
        .unwrap();
    let window = r
        .window(t("2026-01-01T00:00:00Z"), t("2060-01-01T00:00:00Z"))
        .await
        .unwrap();
    assert!(window.instances.len() > 5_000 && window.instances.len() <= MAX_INSTANCES_PER_EVENT);
    assert_eq!(window.damaged.len(), 1);
    assert!(
        window.damaged[0].reason.contains("cut"),
        "{:?}",
        window.damaged
    );
    assert_eq!(window.damaged[0].key, None);
    assert!(
        window.damaged[0].cut,
        "one event over its limit is named and the answer is cut (the contract: `cut`)"
    );
}

#[tokio::test]
async fn a_rule_with_more_starts_than_the_engine_holds_is_named_not_cut_silently() {
    let r = repo().await;
    let mut e = Event::new("Secondly", "2026-11-03T10:00:00", "Etc/UTC", "PT0S");
    e.recurrence_rule =
        Some(serde_json::from_value(json!({"frequency":"minutely","interval":5})).unwrap());
    r.create(e).await.unwrap();
    let window = r
        .window(t("2026-11-03T10:00:00Z"), t("2027-10-20T10:00:00Z"))
        .await
        .unwrap();
    assert!(window.instances.is_empty(), "{}", window.instances.len());
    assert_eq!(window.damaged.len(), 1);
    assert!(window.damaged[0].cut, "{:?}", window.damaged);
    assert!(
        window.damaged[0].reason.contains("65535"),
        "{:?}",
        window.damaged
    );
}

#[tokio::test]
async fn an_event_above_the_size_limit_is_refused() {
    let r = repo().await;
    let mut e = Event::new("Big", "2026-11-03T10:00:00", "Etc/UTC", "PT1H");
    e.description = Some("x".repeat(MAX_EVENT_BYTES));
    let got = r.create(e).await;
    assert!(matches!(got, Err(Error::TooLarge(_))), "{got:?}");
}

#[tokio::test]
async fn a_window_too_long_for_one_event_names_that_event_and_cuts_the_answer() {
    let r = repo().await;
    let mut e = Event::new("Minutely", "2026-12-01T00:00:00", "Etc/UTC", "PT1M");
    e.recurrence_rule =
        Some(serde_json::from_value(json!({"frequency": "minutely", "interval": 5})).unwrap());
    r.create(e).await.unwrap();
    let w = r
        .window(t("2026-12-01T00:00:00Z"), t("2027-06-20T00:00:00Z"))
        .await
        .unwrap();
    // 200 days of 288 starts are 57 600 instances, more than one event may give
    assert!(w.instances.len() > 5_000 && w.instances.len() <= 10_000);
    assert!(w.damaged.iter().all(|d| d.cut), "{:?}", w.damaged);
    assert_eq!(w.damaged.len(), 1);
}

/// Several events that each fit, and together do not: that is a cut of the answer (the events cut
/// by the others carry no other fault).
#[tokio::test]
async fn events_that_together_pass_the_answer_budget_are_a_cut() {
    let r = repo().await;
    for i in 0..4 {
        let mut e = Event::new("Often", "2026-12-01T00:00:00", "Etc/UTC", "PT1M");
        e.uid = format!("often-{i}");
        e.recurrence_rule =
            Some(serde_json::from_value(json!({"frequency": "minutely", "interval": 5})).unwrap());
        r.create(e).await.unwrap();
    }
    let w = r
        .window(t("2026-12-01T00:00:00Z"), t("2027-01-01T00:00:00Z"))
        .await
        .unwrap();
    assert!(w.damaged.iter().any(|d| d.cut), "{:?}", w.damaged);
    assert!(
        w.damaged.iter().all(|d| d.cut),
        "no event is damaged by itself: {:?}",
        w.damaged
    );
}
