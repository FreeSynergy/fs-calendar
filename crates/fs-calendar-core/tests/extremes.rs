//! Times at the edge of what the time type holds: a question answers, it never panics.

mod common;

use chrono::{DateTime, Utc};
use common::repo;
use fs_calendar_core::*;
use serde_json::json;

#[tokio::test]
async fn a_window_from_the_smallest_to_the_largest_instant_answers() {
    let r = repo().await;
    let mut e = Event::new("Daily", "2026-11-01T09:00:00", "Europe/Berlin", "PT1H");
    e.recurrence_rule =
        Some(serde_json::from_value(json!({"frequency": "weekly", "count": 3})).unwrap());
    r.create(e).await.unwrap();
    let w = r
        .window(DateTime::<Utc>::MIN_UTC, DateTime::<Utc>::MAX_UTC)
        .await
        .unwrap();
    assert_eq!(w.instances.len(), 3, "{:?}", w.damaged);
    let w = r
        .window(DateTime::<Utc>::MAX_UTC, DateTime::<Utc>::MAX_UTC)
        .await
        .unwrap();
    assert_eq!(w.instances, [] as [fs_calendar_core::Instance; 0]);
    let w = r
        .window(DateTime::<Utc>::MIN_UTC, DateTime::<Utc>::MIN_UTC)
        .await
        .unwrap();
    assert_eq!(w.instances, [] as [fs_calendar_core::Instance; 0]);
}

#[tokio::test]
async fn upcoming_with_an_absurd_number_of_days_reaches_the_edge_and_never_panics() {
    let r = repo().await;
    let now = utc("2026-11-07T00:00:00Z").unwrap();
    let mut e = Event::new("Later", "2026-11-10T09:00:00", "Etc/UTC", "PT1H");
    e.uid = "later".into();
    r.create(e).await.unwrap();
    for days in [i64::MAX, 400_000_000] {
        let got = r.upcoming(now, days).await.unwrap();
        assert_eq!(got.len(), 1, "{days} days");
    }
    // a negative number of days is a window that ends where it starts: nothing, not a panic
    assert_eq!(
        r.upcoming(now, i64::MIN).await.unwrap(),
        [] as [fs_calendar_core::Instance; 0]
    );
    let calendar: &dyn Calendar = &r;
    let at_the_edge = calendar
        .upcoming(DateTime::<Utc>::MAX_UTC, 1)
        .await
        .unwrap();
    assert_eq!(at_the_edge, [] as [fs_calendar_core::Instance; 0]);
}

#[tokio::test]
async fn an_unbounded_series_asked_at_the_edge_is_named_not_a_panic() {
    let r = repo().await;
    let mut e = Event::new("Daily", "2026-11-01T09:00:00", "Etc/UTC", "PT1H");
    e.recurrence_rule = Some(serde_json::from_value(json!({"frequency": "daily"})).unwrap());
    r.create(e).await.unwrap();
    let w = r
        .window(
            DateTime::<Utc>::MAX_UTC - chrono::Duration::days(2),
            DateTime::<Utc>::MAX_UTC,
        )
        .await
        .unwrap();
    assert_eq!(w.instances, [] as [fs_calendar_core::Instance; 0]);
}
