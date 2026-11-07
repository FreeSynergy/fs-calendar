//! An override or extra instance before the series start is found in its own week.

mod common;

use common::{repo, t};
use fs_calendar_core::*;
use serde_json::json;

fn series() -> Event {
    let mut e = Event::new("Weekly", "2026-11-10T10:00:00", "Etc/UTC", "PT1H");
    e.recurrence_rule =
        Some(serde_json::from_value(json!({"frequency":"weekly","count":3})).unwrap());
    e.recurrence_overrides.insert(
        "2026-11-17T10:00:00".into(),
        json!({"start": "2026-11-03T10:00:00", "title": "Earlier"}),
    );
    e.recurrence_overrides
        .insert("2026-11-02T08:00:00".into(), json!({"title": "Extra"}));
    e
}

#[tokio::test]
async fn the_store_offers_the_series_in_the_week_of_its_earliest_instance() {
    let r = repo().await;
    let made = r.create(series()).await.unwrap();
    let week = r
        .occurrences(t("2026-11-02T00:00:00Z"), t("2026-11-09T00:00:00Z"))
        .await
        .unwrap();
    let got: Vec<_> = week
        .iter()
        .map(|i| (i.occurrence.event.display_title(), i.occurrence.start_utc))
        .collect();
    assert_eq!(
        got,
        [
            ("Extra".to_string(), t("2026-11-02T08:00:00Z")),
            ("Earlier".to_string(), t("2026-11-03T10:00:00Z")),
        ]
    );
    assert_eq!(
        made.envelope.start_time,
        Some(t("2026-11-02T08:00:00Z")),
        "the envelope starts at the earliest instance"
    );
    assert_eq!(made.index["start_utc"], ["2026-11-02T08:00:00Z"]);
    let wide = r
        .occurrences(t("2026-11-01T00:00:00Z"), t("2026-12-01T00:00:00Z"))
        .await
        .unwrap();
    assert_eq!(wide.len(), 4);
}

#[tokio::test]
async fn an_excluded_early_key_does_not_pull_the_start_back() {
    let r = repo().await;
    let mut e = series();
    e.recurrence_overrides.clear();
    e.recurrence_overrides
        .insert("2026-11-01T10:00:00".into(), json!({"excluded": true}));
    let made = r.create(e).await.unwrap();
    assert_eq!(made.envelope.start_time, Some(t("2026-11-10T10:00:00Z")));
}

/// `timeZone: null` takes the zone off the instance: it is floating, which the repository reads in its
/// own zone (UTC). The envelope must start there, not at the master's 17:00 UTC.
#[tokio::test]
async fn an_override_that_removes_the_zone_is_found_where_it_then_is() {
    let r = repo().await;
    let mut e = Event::new(
        "Zoned",
        "2026-11-03T09:00:00",
        "America/Los_Angeles",
        "PT1H",
    );
    e.recurrence_rule =
        Some(serde_json::from_value(json!({"frequency": "weekly", "count": 2})).unwrap());
    e.recurrence_overrides
        .insert("2026-11-03T09:00:00".into(), json!({"timeZone": null}));
    let made = r.create(e).await.unwrap();
    assert_eq!(made.envelope.start_time, Some(t("2026-11-03T09:00:00Z")));
    let found = r
        .occurrences(t("2026-11-03T09:00:00Z"), t("2026-11-03T10:00:00Z"))
        .await
        .unwrap();
    assert_eq!(found.len(), 1);
}
