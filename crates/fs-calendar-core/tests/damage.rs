//! A broken override costs itself, never the calendar.

mod common;

use common::{ME, setup, t};
use fs_calendar_core::*;
use serde_json::json;

fn series(title: &str, uid: &str) -> Event {
    let mut e = Event::new(title, "2026-11-03T10:00:00", "Etc/UTC", "PT1H");
    e.uid = uid.into();
    e.recurrence_rule = Some(serde_json::from_value(json!({"frequency":"daily"})).unwrap());
    e
}

fn broken() -> [(&'static str, serde_json::Value); 3] {
    [
        ("duration", json!({"duration": "nonsense"})),
        ("start", json!({"start": "garbage"})),
        ("zone", json!({"timeZone": "Mars/Base"})),
    ]
}

#[tokio::test]
async fn a_broken_override_is_refused_when_it_is_written() {
    for (name, patch) in broken().iter() {
        let r = common::repo().await;
        let mut e = series("S", "s");
        e.recurrence_overrides
            .insert("2026-11-05T10:00:00".into(), patch.clone());
        let got = r.create(e).await;
        assert!(matches!(got, Err(Error::Invalid(_))), "{name}: {got:?}");
    }
    let r = common::repo().await;
    let mut e = series("S", "s");
    e.recurrence_overrides
        .insert("not a time".into(), json!({"title": "x"}));
    assert!(matches!(r.create(e).await, Err(Error::Invalid(_))));
}

#[tokio::test]
async fn an_import_with_a_broken_override_fails_that_event_only() {
    let r = common::repo().await;
    r.create(series("Fine", "fine")).await.unwrap();
    let bad = r#"[{"@type":"Event","version":"2.0","uid":"bad","title":"Bad","start":"2026-11-03T10:00:00","timeZone":"Etc/UTC","duration":"PT1H","recurrenceRule":{"@type":"RecurrenceRule","frequency":"daily"},"recurrenceOverrides":{"2026-11-05T10:00:00":{"duration":"nonsense"}}}]"#;
    let report = r
        .import_text("application/jscalendar+json", bad)
        .await
        .unwrap();
    assert!(
        report.created.is_empty() && report.failed.len() == 1,
        "{report:?}"
    );
    let day = r
        .occurrences(t("2026-11-04T00:00:00Z"), t("2026-11-06T00:00:00Z"))
        .await
        .unwrap();
    assert_eq!(day.len(), 2);
}

/// Put a broken override into the store behind the repository's back, as an older version could have.
async fn inject(patch: &serde_json::Value) -> (CalendarRepo, fs_objects::ObjectId) {
    let (r, store) = setup().await;
    r.create(series("Fine", "fine")).await.unwrap();
    let broken = r.create(series("Broken", "broken")).await.unwrap();
    let mut o = store.get(broken.envelope.id).await.unwrap().unwrap();
    o.content["recurrenceOverrides"] = json!({"2026-11-05T10:00:00": patch});
    store.put(o, ME).await.unwrap();
    (r, broken.envelope.id)
}

#[tokio::test]
async fn a_stored_broken_override_leaves_the_rest_of_the_calendar_and_the_series() {
    for (name, patch) in broken().iter() {
        let (r, id) = inject(patch).await;
        let from = t("2026-11-04T00:00:00Z");
        let to = t("2026-11-07T00:00:00Z");
        let window = r.window(from, to).await.unwrap();
        let titles = |name: &str| {
            window
                .instances
                .iter()
                .filter(|i| i.occurrence.event.display_title() == name)
                .count()
        };
        assert_eq!(titles("Fine"), 3, "{name}");
        assert_eq!(titles("Broken"), 2, "{name}: the 5th is the damaged one");
        assert_eq!(window.damaged.len(), 1, "{name}: {:?}", window.damaged);
        let damage = &window.damaged[0];
        assert_eq!(damage.object, id);
        assert_eq!(damage.key.as_deref(), Some("2026-11-05T10:00:00"));
        assert!(!damage.reason.is_empty());
        assert_eq!(r.occurrences(from, to).await.unwrap().len(), 5, "{name}");
    }
}

#[tokio::test]
async fn an_event_that_cannot_be_read_at_all_is_named_and_the_others_stay() {
    let (r, store) = setup().await;
    r.create(series("Fine", "fine")).await.unwrap();
    let wrecked = r.create(series("Wrecked", "wrecked")).await.unwrap();
    let mut o = store.get(wrecked.envelope.id).await.unwrap().unwrap();
    o.content["timeZone"] = json!("Mars/Base");
    store.put(o, ME).await.unwrap();
    let window = r
        .window(t("2026-11-04T00:00:00Z"), t("2026-11-06T00:00:00Z"))
        .await
        .unwrap();
    assert_eq!(window.instances.len(), 2);
    assert_eq!(window.damaged.len(), 1);
    assert_eq!(window.damaged[0].object, wrecked.envelope.id);
    assert_eq!(window.damaged[0].key, None);
}
