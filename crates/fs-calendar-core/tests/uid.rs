//! One uid is one event, however the UUID is written (urn:uuid:, case, hyphens).

mod common;

use common::repo;
use fs_calendar_core::*;

const V7: &str = "0192f6a0-7c3e-7b1e-8a3d-5f6c7d8e9f01";
const V4: &str = "3f2b8c1e-9d4a-4e6b-a5c7-1d2e3f4a5b6c";

fn event(uid: &str, title: &str) -> Event {
    let mut e = Event::new(title, "2026-12-01T10:00:00", "Europe/Berlin", "PT1H");
    e.uid = uid.to_string();
    e
}

fn spellings(uuid: &str) -> Vec<String> {
    vec![
        uuid.to_string(),
        format!("urn:uuid:{uuid}"),
        format!("URN:UUID:{}", uuid.to_uppercase()),
        uuid.replace('-', ""),
        uuid.to_uppercase(),
        format!(" {uuid} "),
    ]
}

#[tokio::test]
async fn another_spelling_of_the_uid_is_a_conflict_never_an_overwrite() {
    for uuid in [V7, V4] {
        let r = repo().await;
        let first = r
            .create(event(&format!("urn:uuid:{uuid}"), "First"))
            .await
            .unwrap();
        for other in spellings(uuid) {
            let got = r.create(event(&other, "Second")).await;
            assert!(matches!(got, Err(Error::Conflict(_))), "{other}: {got:?}");
            let found = r.get_by_uid(&other).await.unwrap().unwrap();
            assert_eq!(found.envelope.id, first.envelope.id, "{other}");
            assert_eq!(found.envelope.name, "First", "{other}");
        }
        assert_eq!(r.list_events().await.unwrap().len(), 1);
    }
}

#[tokio::test]
async fn an_import_with_another_spelling_skips_instead_of_overwriting() {
    let r = repo().await;
    r.create(event(&format!("urn:uuid:{V7}"), "First"))
        .await
        .unwrap();
    let ics = format!(
        "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:x\r\nBEGIN:VEVENT\r\nUID:{V7}\r\nDTSTAMP:20261101T000000Z\r\nDTSTART:20261201T090000Z\r\nSUMMARY:Second\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n"
    );
    let report = r.import_text("text/calendar", &ics).await.unwrap();
    assert!(report.created.is_empty(), "{report:?}");
    assert_eq!(report.skipped.len(), 1, "{report:?}");
    let json = format!(
        r#"{{"@type":"Event","version":"2.0","uid":"{}","title":"Third","start":"2026-12-01T10:00:00"}}"#,
        V7.to_uppercase()
    );
    let report = r
        .import_text("application/jscalendar+json", &json)
        .await
        .unwrap();
    assert_eq!(report.skipped.len(), 1, "{report:?}");
    let all = r.list_events().await.unwrap();
    assert_eq!((all.len(), all[0].envelope.name.as_str()), (1, "First"));
}

#[tokio::test]
async fn an_update_may_respell_the_uid_but_not_change_it() {
    let r = repo().await;
    let made = r
        .create(event(&format!("urn:uuid:{V4}"), "First"))
        .await
        .unwrap();
    let id = made.envelope.id;
    r.update(id, event(V4, "Renamed")).await.unwrap();
    assert_eq!(r.get(id).await.unwrap().unwrap().envelope.name, "Renamed");
    let other = event(&format!("urn:uuid:{V7}"), "Else");
    assert!(matches!(r.update(id, other).await, Err(Error::Conflict(_))));
}

#[tokio::test]
async fn a_uid_that_is_no_uuid_stays_as_it_is() {
    let r = repo().await;
    r.create(event("Plain-UID", "A")).await.unwrap();
    assert!(r.get_by_uid("Plain-UID").await.unwrap().is_some());
    assert!(r.get_by_uid("plain-uid").await.unwrap().is_none());
    r.create(event("plain-uid", "B")).await.unwrap();
    assert_eq!(r.list_events().await.unwrap().len(), 2);
}
