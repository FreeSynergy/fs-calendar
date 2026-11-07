//! One series cannot make the calendar unanswerable: dense rules without an end are refused when
//! written, a window is walked from its own start, and an event that cannot be walked is named alone.

mod common;

use std::collections::BTreeSet;

use chrono::{Datelike, Duration, NaiveDate, Weekday};
use common::{ME, setup, t};
use fs_calendar_core::*;
use serde_json::json;

fn rule_event(uid: &str, start: &str, rule: serde_json::Value) -> Event {
    let mut e = Event::new(uid, start, "Etc/UTC", "PT1S");
    e.uid = uid.into();
    e.recurrence_rule = Some(serde_json::from_value(rule).unwrap());
    e
}

#[tokio::test]
async fn a_dense_rule_without_an_end_is_refused_when_written() {
    let (r, _) = setup().await;
    for rule in [
        json!({"frequency": "secondly"}),
        json!({"frequency": "minutely"}),
    ] {
        let got = r
            .create(rule_event("dense", "2026-01-01T00:00:00", rule))
            .await;
        assert!(
            matches!(got, Err(Error::RuleTooExpensive(_))),
            "stored: {}",
            got.is_ok()
        );
    }
    // with an end it is a bounded cost; every five minutes without an end is 288 a day
    let bounded = json!({"frequency": "secondly", "count": 1000});
    r.create(rule_event("bounded", "2026-01-01T00:00:00", bounded))
        .await
        .unwrap();
    let five = json!({"frequency": "minutely", "interval": 5});
    r.create(rule_event("five", "2026-01-01T00:00:00", five))
        .await
        .unwrap();
}

#[tokio::test]
async fn a_window_is_walked_from_its_own_start_not_from_the_series_start() {
    let (r, _) = setup().await;
    let five = json!({"frequency": "minutely", "interval": 5});
    r.create(rule_event("five", "2020-01-01T00:00:03", five))
        .await
        .unwrap();
    let w = r
        .window(t("2040-06-01T10:00:00Z"), t("2040-06-01T10:20:00Z"))
        .await
        .unwrap();
    assert!(w.damaged.is_empty(), "{:?}", w.damaged);
    let starts: Vec<_> = w
        .instances
        .iter()
        .map(|i| i.occurrence.start_utc.to_string())
        .collect();
    let all = starts.len();
    assert_eq!(all, 4, "{starts:?}");
    // 2020-01-01T00:00:03 plus a multiple of five minutes
    assert!(starts.iter().all(|s| s.ends_with(":03 UTC")), "{starts:?}");
    assert!(
        starts
            .iter()
            .all(|s| s[14..16].parse::<u32>().unwrap() % 5 == 0),
        "{starts:?}"
    );
}

#[tokio::test]
async fn the_jump_keeps_the_sequence_of_a_weekly_rule_with_an_interval_and_days() {
    let (r, _) = setup().await;
    let rule =
        json!({"frequency": "weekly", "interval": 2, "byDay": [{"day": "mo"}, {"day": "we"}]});
    r.create(rule_event("w", "2020-01-06T09:00:00", rule))
        .await
        .unwrap(); // a Monday
    let from = NaiveDate::from_ymd_opt(2031, 3, 1).unwrap();
    let to = from + Duration::days(60);
    let w = r
        .window(t("2031-03-01T00:00:00Z"), t("2031-04-30T00:00:00Z"))
        .await
        .unwrap();
    let got: BTreeSet<NaiveDate> = w
        .instances
        .iter()
        .map(|i| i.occurrence.start_utc.date_naive())
        .collect();
    let first = NaiveDate::from_ymd_opt(2020, 1, 6).unwrap();
    let expected: BTreeSet<NaiveDate> = (0..60)
        .map(|d| from + Duration::days(d))
        .filter(|d| d < &to && matches!(d.weekday(), Weekday::Mon | Weekday::Wed))
        .filter(|d| ((*d - first).num_days() / 7) % 2 == 0)
        .collect();
    assert_eq!(got, expected);
}

#[tokio::test]
async fn an_event_that_cannot_be_walked_is_named_alone_and_the_rest_answers() {
    let (r, store) = setup().await;
    for (uid, start, rule) in [
        (
            "secondly",
            "2026-01-01T00:00:00",
            json!({"frequency": "yearly"}),
        ),
        (
            "minutely",
            "2025-01-01T00:00:00",
            json!({"frequency": "yearly"}),
        ),
    ] {
        let made = r.create(rule_event(uid, start, rule)).await.unwrap();
        // written behind the core's back, as an older version could have
        let mut o = store.get(made.envelope.id).await.unwrap().unwrap();
        let frequency = if uid == "secondly" {
            "secondly"
        } else {
            "minutely"
        };
        o.content["recurrenceRule"] = json!({"@type": "RecurrenceRule", "frequency": frequency});
        store.put(o, ME).await.unwrap();
    }
    let mut lunch = Event::new("Lunch", "2026-11-09T12:00:00", "Etc/UTC", "PT1H");
    lunch.uid = "lunch".into();
    r.create(lunch).await.unwrap();
    let w = r
        .window(t("2026-11-09T12:00:00Z"), t("2026-11-09T12:00:01Z"))
        .await
        .unwrap();
    let titles: Vec<_> = w
        .instances
        .iter()
        .map(|i| i.occurrence.event.display_title())
        .collect();
    assert!(titles.contains(&"Lunch".to_string()), "{titles:?}");
    assert!(w.damaged.iter().all(|d| !d.cut), "{:?}", w.damaged);
}

/// A rule that cannot be walked even from its window (dense yearly, from the year 500) is named alone.
#[tokio::test]
async fn a_series_too_dense_to_walk_costs_only_itself() {
    let (r, _) = setup().await;
    let dense = json!({"frequency": "yearly",
        "byMonth": (1..=12).map(|m| m.to_string()).collect::<Vec<_>>(),
        "byMonthDay": (1..=31).collect::<Vec<_>>()});
    let made = r
        .create(rule_event("dense", "0500-01-01T00:00:00", dense))
        .await
        .unwrap();
    let mut lunch = Event::new("Lunch", "2026-11-09T12:00:00", "Etc/UTC", "PT1H");
    lunch.uid = "lunch".into();
    r.create(lunch).await.unwrap();
    let w = r
        .window(t("2026-11-09T12:00:00Z"), t("2026-11-09T12:00:01Z"))
        .await
        .unwrap();
    let titles: Vec<_> = w
        .instances
        .iter()
        .map(|i| i.occurrence.event.display_title())
        .collect();
    assert_eq!(titles, ["Lunch"]);
    assert_eq!(w.damaged.len(), 1, "{:?}", w.damaged);
    assert_eq!(w.damaged[0].object, made.envelope.id, "named by its id");
    assert!(
        !w.damaged[0].cut,
        "an event that cannot be walked is not a cut of the answer"
    );
}

/// The series ended before the window: the override inside it is still there, and nothing is damaged.
#[tokio::test]
async fn an_override_after_the_end_of_its_series_is_found() {
    let (r, _) = setup().await;
    let mut e = rule_event(
        "ended",
        "2026-01-01T09:00:00",
        json!({"frequency": "daily", "until": "2026-02-01T00:00:00"}),
    );
    e.recurrence_overrides
        .insert("2026-06-05T09:00:00".into(), json!({"title": "Late"}));
    r.create(e).await.unwrap();
    let w = r
        .window(t("2026-06-01T00:00:00Z"), t("2026-06-10T00:00:00Z"))
        .await
        .unwrap();
    assert!(w.damaged.is_empty(), "{:?}", w.damaged);
    assert_eq!(w.instances.len(), 1);
    assert_eq!(w.instances[0].occurrence.event.display_title(), "Late");
}
