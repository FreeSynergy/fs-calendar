//! A window of instances, a series expanded by the server.

mod common;

use chrono::{TimeZone, Utc};
use common::{Served, event, json};
use fs_calendar_core::{Calendar, Event};

#[tokio::test]
async fn a_series_comes_as_instances_with_their_recurrence_id_and_override_flag() {
    let served = Served::start().await;
    let c = served.client();
    let mut series = json("s1", "Weekly", "2026-12-01T09:00:00");
    series["recurrenceRule"] =
        serde_json::json!({"@type":"RecurrenceRule","frequency":"weekly","count":4});
    series["recurrenceOverrides"] = serde_json::json!({"2026-12-08T09:00:00":{"title":"Moved"}});
    let made = c.create(Event::from_value(series).unwrap()).await.unwrap();
    c.create(event("one", "Single", "2026-12-03T12:00:00"))
        .await
        .unwrap();
    let from = Utc.with_ymd_and_hms(2026, 11, 30, 0, 0, 0).unwrap();
    let to = Utc.with_ymd_and_hms(2026, 12, 31, 0, 0, 0).unwrap();
    let found = c.occurrences(from, to).await.unwrap();
    let ours: Vec<_> = found
        .iter()
        .filter(|i| i.object == made.envelope.id)
        .collect();
    assert_eq!(ours.len(), 4);
    let ids: Vec<&str> = ours
        .iter()
        .map(|i| i.occurrence.recurrence_id.as_str())
        .collect();
    assert_eq!(
        ids,
        [
            "2026-12-01T09:00:00",
            "2026-12-08T09:00:00",
            "2026-12-15T09:00:00",
            "2026-12-22T09:00:00"
        ]
    );
    let single = found
        .iter()
        .find(|i| i.occurrence.event.uid == "one")
        .unwrap();
    assert_eq!(
        single.occurrence.recurrence_id, "2026-12-03T12:00:00",
        "a single event is keyed by its start"
    );
    let moved = ours[1];
    assert!(moved.occurrence.overridden);
    assert_eq!(moved.occurrence.event.title.as_deref(), Some("Moved"));
    assert!(!ours[0].occurrence.overridden);
    // The same question put to the core behind the server gives the same instants.
    let direct = Calendar::occurrences(served.repo.as_ref(), from, to)
        .await
        .unwrap();
    assert_eq!(direct.len(), found.len());
    for (a, b) in direct.iter().zip(&found) {
        assert_eq!(
            (a.object, a.occurrence.start_utc, a.occurrence.end_utc),
            (b.object, b.occurrence.start_utc, b.occurrence.end_utc)
        );
    }
}

#[tokio::test]
async fn a_window_that_ends_before_it_starts_is_refused_in_words() {
    let served = Served::start().await;
    let c = served.client();
    let from = Utc.with_ymd_and_hms(2026, 12, 2, 0, 0, 0).unwrap();
    let to = Utc.with_ymd_and_hms(2026, 12, 1, 0, 0, 0).unwrap();
    assert!(c.occurrences(from, to).await.is_err());
}
