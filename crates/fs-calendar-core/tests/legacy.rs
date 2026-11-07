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
            .all(|d| !d.cut && d.object == made.envelope.id),
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
