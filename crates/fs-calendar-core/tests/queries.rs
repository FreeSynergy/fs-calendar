//! Time-range queries, ordering and the 0.1 time-field migration.

mod common;

use common::{ME, repo, setup, t};
use fs_calendar_core::*;
use serde_json::json;

#[tokio::test]
async fn range_queries_expand_series() {
    let r = repo().await;
    let mut weekly = Event::new("Weekly", "2026-11-03T10:00:00", "Europe/Berlin", "PT1H");
    weekly.recurrence_rule = Some(serde_json::from_value(json!({"frequency":"weekly"})).unwrap());
    weekly
        .recurrence_overrides
        .insert("2026-11-10T10:00:00".into(), json!({"excluded": true}));
    let mut ended = Event::new("Ended", "2026-01-05T10:00:00", "Europe/Berlin", "PT1H");
    ended.recurrence_rule =
        Some(serde_json::from_value(json!({"frequency":"daily","count":3})).unwrap());
    let single = Event::new("Single", "2026-11-12T15:00:00", "Europe/Berlin", "PT2H");
    let past = Event::new("Past", "2026-10-01T15:00:00", "Europe/Berlin", "PT2H");
    for e in [weekly, ended, single, past] {
        r.create(e).await.unwrap();
    }
    let got = r
        .occurrences(t("2026-11-05T00:00:00Z"), t("2026-12-01T00:00:00Z"))
        .await
        .unwrap();
    let names: Vec<_> = got
        .iter()
        .map(|i| {
            (
                i.occurrence.event.display_title(),
                i.occurrence.recurrence_id.clone(),
            )
        })
        .collect();
    assert_eq!(
        names,
        [
            ("Single".to_string(), "2026-11-12T15:00:00".to_string()),
            ("Weekly".into(), "2026-11-17T10:00:00".into()),
            ("Weekly".into(), "2026-11-24T10:00:00".into()),
        ]
    );
    assert!(
        r.upcoming(t("2026-11-04T00:00:00Z"), 5)
            .await
            .unwrap()
            .is_empty(),
        "the 10th is excluded"
    );
    let next = r.upcoming(t("2026-11-04T00:00:00Z"), 9).await.unwrap();
    assert_eq!(next.len(), 1);
    assert_eq!(next[0].occurrence.event.display_title(), "Single");
    // far future still finds the unbounded series
    let far = r
        .occurrences(t("2030-01-01T00:00:00Z"), t("2030-01-15T00:00:00Z"))
        .await
        .unwrap();
    assert_eq!(far.len(), 2);
}

#[tokio::test]
async fn envelope_times_and_store_side_range_queries() {
    let (r, store) = setup().await;
    let ev = |title: &str, start: &str, tz: &str| Event::new(title, start, tz, "PT1H");
    let one = r
        .create(ev("one", "2026-11-24T10:00:00", "Europe/Berlin"))
        .await
        .unwrap();
    assert_eq!(one.envelope.start_time, Some(t("2026-11-24T09:00:00Z")));
    assert_eq!(one.envelope.end_time, Some(t("2026-11-24T10:00:00Z")));
    // bounded series: end = last instance; unbounded: FOREVER
    let mut weekly = ev("weekly", "2026-11-02T10:00:00", "Europe/Berlin");
    weekly.recurrence_rule =
        serde_json::from_value(json!({"@type":"RecurrenceRule","frequency":"weekly","count":3}))
            .unwrap();
    let w = r.create(weekly).await.unwrap();
    assert_eq!(w.envelope.end_time, Some(t("2026-11-16T10:00:00Z")));
    let mut forever = ev("forever", "2026-01-05T08:00:00", "Europe/Berlin");
    forever.recurrence_rule =
        serde_json::from_value(json!({"@type":"RecurrenceRule","frequency":"daily"})).unwrap();
    let f = r.create(forever).await.unwrap();
    assert_eq!(f.envelope.end_time, Some(t("9999-12-31T23:59:59Z")));
    r.create(ev("later", "2027-05-01T10:00:00", "Europe/Berlin"))
        .await
        .unwrap();
    // JSON form uses the AS2 names
    let j = serde_json::to_value(&one.envelope).unwrap();
    assert_eq!(j["startTime"], "2026-11-24T09:00:00Z");

    // window 2026-11-24 only: "one" (store-selected) plus the unbounded daily; not the finished weekly or the future one
    let inst = r
        .occurrences(t("2026-11-24T00:00:00Z"), t("2026-11-25T00:00:00Z"))
        .await
        .unwrap();
    let titles: Vec<_> = inst
        .iter()
        .map(|i| i.occurrence.event.display_title().to_string())
        .collect();
    assert_eq!(titles, ["forever", "one"]);
    // the store narrows: count what the range query alone returns
    let q = fs_store::Query::new()
        .of_type("Event")
        .time(
            fs_store::TimeField::Start,
            fs_store::Op::Lt,
            t("2026-11-25T00:00:00Z"),
        )
        .time(
            fs_store::TimeField::End,
            fs_store::Op::Ge,
            t("2026-11-24T00:00:00Z"),
        );
    assert_eq!(
        store.count(&q).await.unwrap(),
        2,
        "weekly ended 16 Nov, later starts in 2027"
    );
    // list ordered by first start in the store
    let order: Vec<_> = r
        .list_events()
        .await
        .unwrap()
        .into_iter()
        .map(|o| o.envelope.name)
        .collect();
    assert_eq!(order, ["forever", "weekly", "one", "later"]);
    // an update moves the envelope times
    let mut moved = r.get_event(one.envelope.id).await.unwrap().unwrap();
    moved.start = Some("2026-12-24T10:00:00".into());
    let moved = r.update(one.envelope.id, moved).await.unwrap();
    assert_eq!(moved.envelope.start_time, Some(t("2026-12-24T09:00:00Z")));
}

#[tokio::test]
async fn migrate_time_fields_fills_0_1_objects() {
    let (r, store) = setup().await;
    let o = r
        .create(Event::new(
            "old",
            "2026-11-24T10:00:00",
            "Europe/Berlin",
            "PT1H",
        ))
        .await
        .unwrap();
    // simulate a 0.1 object: same content and index, no envelope times
    let mut legacy = o.clone();
    legacy.envelope.start_time = None;
    legacy.envelope.end_time = None;
    store.put(legacy, ME).await.unwrap();
    assert!(
        r.occurrences(t("2026-11-24T00:00:00Z"), t("2026-11-25T00:00:00Z"))
            .await
            .unwrap()
            .is_empty(),
        "invisible to range queries until migrated"
    );
    assert_eq!(r.migrate_time_fields().await.unwrap(), 1);
    assert_eq!(r.migrate_time_fields().await.unwrap(), 0, "idempotent");
    assert_eq!(
        r.occurrences(t("2026-11-24T00:00:00Z"), t("2026-11-25T00:00:00Z"))
            .await
            .unwrap()
            .len(),
        1
    );
}
