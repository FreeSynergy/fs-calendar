//! The ends of the calendar: years near 0 and 9999 are stored or refused by name, never a panic, and
//! what is stored is found.

mod common;

use common::{repo, t};
use fs_calendar_core::*;

fn event(uid: &str, start: &str, zone: &str, duration: &str) -> Event {
    let mut e = Event::new(uid, start, zone, duration);
    e.uid = uid.into();
    e
}

#[tokio::test]
async fn an_event_near_either_edge_is_stored_with_its_real_time_and_found() {
    let r = repo().await;
    for (uid, start, zone, from, to) in [
        (
            "first",
            "0001-01-01T00:00:00",
            "Pacific/Auckland",
            "0000-12-31T00:00:00Z",
            "0001-01-01T00:00:00Z",
        ),
        (
            "last",
            "9998-12-31T23:00:00",
            "Pacific/Pago_Pago",
            "9999-01-01T09:00:00Z",
            "9999-01-01T11:00:00Z",
        ),
    ] {
        let made = r.create(event(uid, start, zone, "PT1H")).await.unwrap();
        for key in ["start_utc", "end_utc"] {
            let value = &made.index[key][0];
            assert!(
                value.len() == 20 && value.as_bytes()[0].is_ascii_digit(),
                "{uid} {key}: {value}"
            );
        }
        let found = r.occurrences(t(from), t(to)).await.unwrap();
        assert_eq!(found.len(), 1, "{uid}: {found:?}");
        assert_eq!(found[0].object, made.envelope.id, "{uid}");
    }
}

#[tokio::test]
async fn a_start_at_the_edge_is_refused_with_its_reason_not_moved() {
    let r = repo().await;
    for (start, zone) in [
        ("0000-01-01T00:00:00", "Pacific/Auckland"),
        ("9999-12-31T23:00:00", "Pacific/Pago_Pago"),
        ("9999-12-31T23:59:59", "Etc/UTC"),
    ] {
        let got = r.create(event("edge", start, zone, "PT1H")).await;
        let Err(Error::Invalid(issues)) = got else {
            panic!("{start}: {got:?}");
        };
        assert_eq!(issues[0].path, "/start", "{start}");
    }
}

#[tokio::test]
async fn an_endless_series_near_the_end_and_a_huge_duration_do_not_panic() {
    let r = repo().await;
    let mut series = event("series", "9998-01-01T10:00:00", "Etc/UTC", "PT1H");
    series.recurrence_rule =
        Some(serde_json::from_value(serde_json::json!({"frequency": "yearly"})).unwrap());
    let _ = r.create(series).await;
    let _ = r
        .create(event("long", "2026-01-01T10:00:00", "Etc/UTC", "P3000000D"))
        .await;
    let _ = r
        .window(t("9998-01-01T00:00:00Z"), t("9999-12-31T00:00:00Z"))
        .await;
}
