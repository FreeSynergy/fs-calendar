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
async fn a_window_far_from_the_series_start_is_refused_quickly_and_named() {
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
    assert_eq!(window.instances.len(), 1, "the single event stays");
    assert_eq!(window.damaged.len(), 1, "{:?}", window.damaged);
    assert!(
        window.damaged[0].reason.contains("series start"),
        "{:?}",
        window.damaged
    );
}

#[tokio::test]
async fn more_instances_than_the_limit_are_cut_and_the_cut_is_named() {
    let r = repo().await;
    r.create(daily("Daily", "2026-01-01T09:00:00"))
        .await
        .unwrap();
    let window = r
        .window(t("2026-01-01T00:00:00Z"), t("2060-01-01T00:00:00Z"))
        .await
        .unwrap();
    assert_eq!(window.instances.len(), MAX_INSTANCES_PER_EVENT);
    assert_eq!(window.damaged.len(), 1);
    assert!(
        window.damaged[0].reason.contains("cut"),
        "{:?}",
        window.damaged
    );
    assert_eq!(window.damaged[0].key, None);
}

#[tokio::test]
async fn a_rule_with_more_starts_than_the_engine_holds_is_named_not_cut_silently() {
    let r = repo().await;
    let mut e = Event::new("Secondly", "2026-11-03T10:00:00", "Etc/UTC", "PT0S");
    e.recurrence_rule = Some(serde_json::from_value(json!({"frequency":"secondly"})).unwrap());
    r.create(e).await.unwrap();
    let window = r
        .window(t("2026-11-03T10:00:00Z"), t("2026-11-05T10:00:00Z"))
        .await
        .unwrap();
    assert!(window.instances.is_empty(), "{}", window.instances.len());
    assert_eq!(window.damaged.len(), 1);
}

#[tokio::test]
async fn an_event_above_the_size_limit_is_refused() {
    let r = repo().await;
    let mut e = Event::new("Big", "2026-11-03T10:00:00", "Etc/UTC", "PT1H");
    e.description = Some("x".repeat(MAX_EVENT_BYTES));
    let got = r.create(e).await;
    assert!(matches!(got, Err(Error::TooLarge(_))), "{got:?}");
}
