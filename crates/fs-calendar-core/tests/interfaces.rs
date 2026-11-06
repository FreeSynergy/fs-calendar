//! The program is usable through its traits alone: `Calendar`, `Importer`, `Exporter`,
//! `RecurrenceEngine`, `ZoneSource` and the participation state machine (ADR 0043).

mod common;

use std::sync::Arc;

use chrono::{DateTime, NaiveDateTime, Utc};
use common::{repo, t};
use fs_calendar_core::format::{Parsed, Part, Split};
use fs_calendar_core::time::{BundledTzdb, Zone};
use fs_calendar_core::{
    Calendar, CalendarObject, Event, Exporter, IcsFormat, Importer, ParticipationStatus,
    RecurrenceEngine, RecurrenceRule, Response, Result,
};

#[tokio::test]
async fn calendar_works_as_a_trait_object() {
    let cal: Arc<dyn Calendar> = Arc::new(repo().await);
    let e = Event::new("Planning", "2026-11-24T10:00:00", "Europe/Berlin", "PT1H");
    let o = cal.create(CalendarObject::Event(e)).await.unwrap();
    let id = o.envelope.id;
    cal.invite(id, &[("Ben", "ben@example.org")]).await.unwrap();
    cal.respond(id, "ben@example.org", Response::Accept, None)
        .await
        .unwrap();
    assert_eq!(cal.invitation_log(id).await.unwrap().len(), 2);
    let next = cal
        .occurrences(t("2026-11-01T00:00:00Z"), t("2026-12-01T00:00:00Z"))
        .await
        .unwrap();
    assert_eq!(next.len(), 1);
    assert!(cal.export_ics(&[id]).await.unwrap().contains("Planning"));
    assert_eq!(IcsFormat.output_media_type(), "text/calendar");
}

/// An engine that repeats every rule daily, three times: proves the engine is swappable.
struct ThreeDays;

impl RecurrenceEngine for ThreeDays {
    fn starts(
        &self,
        _rule: &RecurrenceRule,
        start: NaiveDateTime,
        _zone: &Zone,
        _after: DateTime<Utc>,
        _before: DateTime<Utc>,
    ) -> Result<Vec<NaiveDateTime>> {
        Ok((1..3).map(|i| start + chrono::Duration::days(i)).collect())
    }
}

#[tokio::test]
async fn the_recurrence_engine_is_swappable() {
    let r = repo().await.with_recurrence_engine(Arc::new(ThreeDays));
    let mut e = Event::new("Series", "2026-11-24T10:00:00", "Etc/UTC", "PT1H");
    e.recurrence_rule =
        Some(serde_json::from_value(serde_json::json!({"frequency": "daily"})).unwrap());
    r.create(e).await.unwrap();
    let all = r
        .occurrences(t("2026-11-01T00:00:00Z"), t("2027-01-01T00:00:00Z"))
        .await
        .unwrap();
    assert_eq!(all.len(), 3);
}

struct Plain;

impl Importer for Plain {
    fn media_type(&self) -> &'static str {
        "text/plain"
    }
    fn split(&self, text: &str) -> Result<Split> {
        let parts = text
            .lines()
            .map(|l| Part {
                uid: None,
                text: l.to_string(),
            })
            .collect();
        Ok(Split {
            parts,
            notes: vec![],
        })
    }
    fn parse(&self, part: &Part) -> Result<Parsed> {
        let e = Event::new(part.text.clone(), "2026-11-24T10:00:00", "Etc/UTC", "PT1H");
        Ok(Parsed {
            objects: vec![e.into()],
            original: None,
            notes: vec![],
        })
    }
}

#[tokio::test]
async fn a_new_format_plugs_in_as_an_importer() {
    let r = repo().await;
    let rep = r.import(&Plain, "One\nTwo").await.unwrap();
    assert_eq!(rep.created.len(), 2);
    assert_eq!(r.list_events().await.unwrap().len(), 2);
}

#[test]
fn participation_status_is_a_state_machine() {
    use ParticipationStatus::*;
    assert_eq!(ParticipationStatus::INITIAL, NeedsAction);
    assert_eq!(NeedsAction.respond(Response::Accept), Accepted);
    assert_eq!(Accepted.respond(Response::Reject), Declined);
    assert_eq!(Declined.respond(Response::TentativeAccept), Tentative);
    for n in ParticipationStatus::NAMES {
        assert_eq!(ParticipationStatus::parse(n).unwrap().as_str(), n);
    }
    assert_eq!(Response::Reject.status(), "declined");
    assert!(ParticipationStatus::parse("maybe").is_none());
}

#[test]
fn zone_sources_agree_on_zone_names() {
    let b = BundledTzdb::new();
    assert_eq!(
        Zone::parse_in(&b, "Asia/Kolkata").unwrap().name(),
        "Asia/Kolkata"
    );
}
