//! Data written before a limit: it costs itself, never the rest of the calendar, and an edit that does
//! not touch the rule is not refused for it.

mod common;

use common::{ME, setup, t};
use fs_calendar_core::*;
use serde_json::json;

#[tokio::test]
async fn a_rule_that_is_too_dense_in_the_store_is_named_alone_and_a_title_can_change() {
    let (r, store) = setup().await;
    let mut e = Event::new("Every second", "2026-01-01T00:00:00", "Etc/UTC", "PT1S");
    e.uid = "dense".into();
    e.recurrence_rule = Some(serde_json::from_value(json!({"frequency": "daily"})).unwrap());
    let made = r.create(e).await.unwrap();
    let mut o = store.get(made.envelope.id).await.unwrap().unwrap();
    o.content["recurrenceRule"] = json!({"@type": "RecurrenceRule", "frequency": "secondly"});
    store.put(o, ME).await.unwrap();
    let mut lunch = Event::new("Lunch", "2026-11-09T12:00:00", "Etc/UTC", "PT1H");
    lunch.uid = "lunch".into();
    r.create(lunch).await.unwrap();

    // a whole day: 86 400 starts of a rule that could not be written today
    let w = r
        .window(t("2026-11-09T00:00:00Z"), t("2026-11-10T00:00:00Z"))
        .await
        .unwrap();
    let titles: Vec<_> = w
        .instances
        .iter()
        .map(|i| i.occurrence.event.display_title())
        .collect();
    assert!(titles.contains(&"Lunch".to_string()), "{titles:?}");
    assert!(
        w.damaged
            .iter()
            .all(|d| d.cut && d.object == made.envelope.id),
        "{:?}",
        w.damaged
    );
    assert_eq!(w.damaged.len(), 1, "{:?}", w.damaged);

    // a new title does not touch the rule, so the old rule does not refuse it
    let mut again = r.get_event(made.envelope.id).await.unwrap().unwrap();
    again.title = Some("Renamed".into());
    r.update(made.envelope.id, again).await.unwrap();
    // changing the rule itself is held to today's limits
    let mut worse = r.get_event(made.envelope.id).await.unwrap().unwrap();
    worse.recurrence_rule = Some(serde_json::from_value(json!({"frequency": "minutely"})).unwrap());
    assert!(matches!(
        r.update(made.envelope.id, worse).await,
        Err(Error::RuleTooExpensive(_))
    ));
}

/// A series with an end, older than the limits, whose end cannot be computed: a window of a day names it
/// alone, and a new title is not refused for it.
#[tokio::test]
async fn a_dense_series_with_an_until_in_the_store_costs_only_itself() {
    let (r, store) = setup().await;
    let mut e = Event::new("Every second", "2026-01-01T00:00:00", "Etc/UTC", "PT1S");
    e.uid = "ended".into();
    e.recurrence_rule = Some(
        serde_json::from_value(json!({"frequency": "daily", "until": "2026-02-01T00:00:00"}))
            .unwrap(),
    );
    let made = r.create(e).await.unwrap();
    let mut o = store.get(made.envelope.id).await.unwrap().unwrap();
    o.content["recurrenceRule"] =
        json!({"@type": "RecurrenceRule", "frequency": "secondly", "until": "2027-01-01T00:00:00"});
    store.put(o, ME).await.unwrap();
    let mut lunch = Event::new("Lunch", "2026-01-09T12:00:00", "Etc/UTC", "PT1H");
    lunch.uid = "lunch".into();
    r.create(lunch).await.unwrap();
    let w = r
        .window(t("2026-01-09T00:00:00Z"), t("2026-01-10T00:00:00Z"))
        .await
        .unwrap();
    let titles: Vec<_> = w
        .instances
        .iter()
        .map(|i| i.occurrence.event.display_title())
        .collect();
    assert!(titles.contains(&"Lunch".to_string()), "{titles:?}");
    assert!(
        w.damaged
            .iter()
            .all(|d| d.cut && d.object == made.envelope.id),
        "{:?}",
        w.damaged
    );
    let mut again = r.get_event(made.envelope.id).await.unwrap().unwrap();
    again.title = Some("Renamed".into());
    r.update(made.envelope.id, again).await.unwrap();
}

/// A legal rule over a month stays below the limit of instances of one event, so it never has to be cut.
#[tokio::test]
async fn a_rule_that_can_be_written_fills_a_month_below_the_limit_of_one_event() {
    use fs_calendar_core::limits::{MAX_INSTANCES_PER_EVENT, MAX_UNBOUNDED_STARTS_PER_DAY};
    assert!(MAX_UNBOUNDED_STARTS_PER_DAY * 31 <= MAX_INSTANCES_PER_EVENT as u64);
    let (r, _) = setup().await;
    let mut e = Event::new("Often", "2026-01-01T00:00:00", "Etc/UTC", "PT1S");
    e.uid = "often".into();
    e.recurrence_rule =
        Some(serde_json::from_value(json!({"frequency": "minutely", "interval": 5})).unwrap());
    r.create(e).await.unwrap();
    let mut e = Event::new("Too often", "2026-01-01T00:00:00", "Etc/UTC", "PT1S");
    e.uid = "too-often".into();
    e.recurrence_rule =
        Some(serde_json::from_value(json!({"frequency": "minutely", "interval": 4})).unwrap());
    assert!(matches!(r.create(e).await, Err(Error::RuleTooExpensive(_))));
    let w = r
        .window(t("2026-03-01T00:00:00Z"), t("2026-04-01T00:00:00Z"))
        .await
        .unwrap();
    assert_eq!(w.instances.len(), 31 * 288);
    assert!(w.damaged.is_empty(), "{:?}", w.damaged);
}
