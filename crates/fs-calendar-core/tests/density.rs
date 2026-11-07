//! A rule is held to what it costs: refused when written if one period gives too many starts, and
//! asked with a hard budget when read.

mod common;

use std::time::{Duration, Instant};

use common::{ME, setup, t};
use fs_calendar_core::*;
use serde_json::json;

fn with_rule(uid: &str, rule: serde_json::Value) -> Event {
    let mut e = Event::new(uid, "2026-01-01T09:00:00", "Etc/UTC", "PT1S");
    e.uid = uid.into();
    e.recurrence_rule = Some(serde_json::from_value(rule).unwrap());
    e
}

fn dense() -> serde_json::Value {
    json!({"frequency": "yearly",
        "byMonth": (1..=12).map(|m| m.to_string()).collect::<Vec<_>>(),
        "byMonthDay": (1..=31).collect::<Vec<_>>(),
        "byHour": (0..24).collect::<Vec<_>>(),
        "byMinute": (0..60).collect::<Vec<_>>(),
        "bySecond": (0..60).collect::<Vec<_>>()})
}

#[tokio::test]
async fn a_rule_with_millions_of_starts_a_year_is_refused_when_written_with_a_typed_error() {
    let (r, _) = setup().await;
    let got = r.create(with_rule("dense", dense())).await;
    assert!(
        matches!(got, Err(Error::RuleTooExpensive(_))),
        "stored: {}",
        got.is_ok()
    );
    let got = r
        .import_text(
            "application/jscalendar+json",
            &with_rule("dense2", dense()).to_value().to_string(),
        )
        .await
        .unwrap();
    assert!(got.created.is_empty() && got.failed.len() == 1, "{got:?}");
    assert!(got.failed[0].contains("too expensive"), "{got:?}");
}

#[tokio::test]
async fn ordinary_rules_with_by_parts_are_not_refused() {
    let (r, _) = setup().await;
    let christmas =
        json!({"frequency": "yearly", "byMonth": ["12"], "byMonthDay": [25], "byHour": [9, 17]});
    r.create(with_rule("xmas", christmas)).await.unwrap();
    let first_monday = json!({"frequency": "monthly", "byDay": [{"day": "mo", "nthOfPeriod": 1}]});
    r.create(with_rule("mon", first_monday)).await.unwrap();
    let w = r
        .window(t("2030-12-24T00:00:00Z"), t("2030-12-27T00:00:00Z"))
        .await
        .unwrap();
    assert_eq!(w.instances.len(), 2, "{:?}", w.damaged);
}

#[tokio::test]
async fn a_dense_rule_in_the_store_is_answered_within_a_budget_and_named() {
    let (r, store) = setup().await;
    let made = r
        .create(with_rule("plain", json!({"frequency": "yearly"})))
        .await
        .unwrap();
    let mut o = store.get(made.envelope.id).await.unwrap().unwrap();
    o.content["recurrenceRule"] = dense();
    store.put(o, ME).await.unwrap();
    let began = Instant::now();
    let w = r
        .window(t("2027-01-01T00:00:00Z"), t("2028-01-01T00:00:00Z"))
        .await
        .unwrap();
    assert!(
        began.elapsed() < Duration::from_secs(2),
        "{:?}",
        began.elapsed()
    );
    assert_eq!(w.damaged.len(), 1, "{:?}", w.damaged);
}
