//! Import and export through the server; a format run here on the events the server sends.

mod common;

use common::{Served, event};
use fs_calendar_core::{Calendar, Error, IcsFormat};
use fs_store::ObjectId;

const ICS: &str = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//t//EN\r\nBEGIN:VEVENT\r\nUID:ics-1\r\nDTSTAMP:20261101T000000Z\r\nDTSTART:20261203T100000Z\r\nDTEND:20261203T110000Z\r\nSUMMARY:From a file\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";

#[tokio::test]
async fn an_ics_part_is_imported_and_the_second_time_it_is_skipped() {
    let served = Served::start().await;
    let c = served.client();
    let first = c.import(&IcsFormat, ICS).await.unwrap();
    assert_eq!(first.created.len(), 1, "{first:?}");
    let stored = c.get(first.created[0]).await.unwrap().unwrap();
    assert_eq!(stored.envelope.name, "From a file");
    let second = c.import_ics(ICS).await.unwrap();
    assert!(second.created.is_empty());
    assert_eq!(second.skipped.len(), 1);
}

#[tokio::test]
async fn a_media_type_nobody_knows_is_refused_and_nothing_is_stored() {
    let served = Served::start().await;
    let c = served.client();
    let refused = c.import_text("text/plain", "x").await;
    assert!(refused.is_err(), "{refused:?}");
    assert!(c.list_events().await.unwrap().is_empty());
}

#[tokio::test]
async fn export_runs_the_given_format_over_the_events_read_from_the_server() {
    let served = Served::start().await;
    let c = served.client();
    let a = c
        .create(event("x1", "Alpha", "2026-12-01T09:00:00"))
        .await
        .unwrap();
    let b = c
        .create(event("x2", "Beta", "2026-12-02T09:00:00"))
        .await
        .unwrap();
    let text = c
        .export(&IcsFormat, &[a.envelope.id, b.envelope.id])
        .await
        .unwrap();
    assert!(
        text.contains("SUMMARY:Alpha") && text.contains("SUMMARY:Beta"),
        "{text}"
    );
    // The same text as the server's own, but for the id it gives the calendar as a whole each time.
    let theirs = c.export_ics(&[a.envelope.id, b.envelope.id]).await.unwrap();
    let plain = |t: &str| {
        t.lines()
            .filter(|l| !l.starts_with("UID:urn:uuid"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    assert_eq!(plain(&text), plain(&theirs));
    let again = c
        .export_text(&[a.envelope.id], "text/calendar")
        .await
        .unwrap();
    assert!(again.contains("Alpha") && !again.contains("Beta"));
}

#[tokio::test]
async fn exporting_what_is_not_there_is_not_found() {
    let served = Served::start().await;
    let ghost = ObjectId::new();
    let missing = served.client().export(&IcsFormat, &[ghost]).await;
    assert!(
        matches!(missing, Err(Error::NotFound(id)) if id == ghost),
        "{missing:?}"
    );
}
