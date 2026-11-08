//! An import never moves an event to another time: a zone no one answers to is refused, by name.

mod common;

use common::repo;
use fs_calendar_core::*;

fn ics(body: &str) -> String {
    format!(
        "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:x\r\nBEGIN:VEVENT\r\nUID:z1\r\nDTSTAMP:20261101T000000Z\r\nSUMMARY:Zoned\r\n{body}END:VEVENT\r\nEND:VCALENDAR\r\n"
    )
}

async fn refused(body: &str) -> String {
    let r = repo().await;
    let report = r.import_text("text/calendar", &ics(body)).await.unwrap();
    assert!(report.created.is_empty(), "stored: {report:?}");
    assert!(r.list_events().await.unwrap().is_empty(), "{report:?}");
    assert_eq!(report.failed.len(), 1, "{report:?}");
    report.failed[0].clone()
}

#[tokio::test]
async fn an_unknown_zone_on_the_start_is_named_not_turned_into_utc() {
    let text =
        refused("DTSTART;TZID=Not/AZone:20261130T090000\r\nDTEND:20261130T100000Z\r\n").await;
    assert!(text.contains("z1") && text.contains("Not/AZone"), "{text}");
}

#[tokio::test]
async fn an_unknown_start_with_a_known_end_does_not_become_an_event_of_length_zero() {
    let text = refused(
        "DTSTART;TZID=Not/AZone:20261130T090000\r\nDTEND;TZID=Europe/Berlin:20261130T110000\r\n",
    )
    .await;
    assert!(text.contains("Not/AZone"), "{text}");
}

#[tokio::test]
async fn an_unknown_end_zone_is_named_too() {
    let text = refused(
        "DTSTART;TZID=Europe/Berlin:20261130T090000\r\nDTEND;TZID=Not/AZone:20261130T110000\r\n",
    )
    .await;
    assert!(text.contains("Not/AZone"), "{text}");
}

#[tokio::test]
async fn an_exception_with_an_unknown_zone_does_not_appear_twice_under_a_shifted_key() {
    let r = repo().await;
    let text = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:x\r\nBEGIN:VEVENT\r\nUID:s1\r\nDTSTAMP:20261101T000000Z\r\nDTSTART;TZID=Not/AZone:20261130T100000\r\nDURATION:PT1H\r\nRRULE:FREQ=DAILY;COUNT=3\r\nSUMMARY:S\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nUID:s1\r\nDTSTAMP:20261101T000000Z\r\nRECURRENCE-ID;TZID=Not/AZone:20261201T100000\r\nDTSTART;TZID=Not/AZone:20261201T110000\r\nDURATION:PT1H\r\nSUMMARY:Moved\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
    let report = r.import_text("text/calendar", text).await.unwrap();
    assert!(
        report.created.is_empty() && report.failed.len() == 1,
        "{report:?}"
    );
    assert!(report.failed[0].contains("Not/AZone"), "{report:?}");
    let from = utc("2026-11-29T00:00:00Z").unwrap();
    let to = utc("2026-12-05T00:00:00Z").unwrap();
    assert_eq!(
        r.occurrences(from, to).await.unwrap(),
        [] as [fs_calendar_core::Instance; 0]
    );
}

#[tokio::test]
async fn a_known_zone_and_a_floating_time_still_import() {
    let r = repo().await;
    let known = ics(
        "DTSTART;TZID=Europe/Berlin:20261130T090000\r\nDTEND;TZID=Europe/Berlin:20261130T100000\r\n",
    );
    let report = r.import_text("text/calendar", &known).await.unwrap();
    assert_eq!(report.created.len(), 1, "{report:?}");
    let stored = r.get(report.created[0]).await.unwrap().unwrap();
    assert_eq!(stored.content["timeZone"], "Europe/Berlin");
    let r = repo().await;
    let floating = ics("DTSTART:20261130T090000\r\nDTEND:20261130T100000\r\n");
    let report = r.import_text("text/calendar", &floating).await.unwrap();
    assert_eq!(report.created.len(), 1, "{report:?}");
}

#[tokio::test]
async fn one_refused_event_costs_itself_and_not_the_others_of_the_file() {
    let r = repo().await;
    let two = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:x\r\nBEGIN:VEVENT\r\nUID:bad\r\nDTSTAMP:20261101T000000Z\r\nDTSTART;TZID=Not/AZone:20261130T090000\r\nSUMMARY:Bad\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nUID:good\r\nDTSTAMP:20261101T000000Z\r\nDTSTART:20261130T090000Z\r\nSUMMARY:Good\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
    let report = r.import_text("text/calendar", two).await.unwrap();
    assert_eq!(
        (report.created.len(), report.failed.len()),
        (1, 1),
        "{report:?}"
    );
    assert!(report.failed[0].starts_with("bad"), "{report:?}");
}
