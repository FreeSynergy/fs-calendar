//! `importText` / `exportText` of the contract: the media type picks the format; an unknown one
//! is an error.

mod common;

use common::{ICS, repo};
use fs_calendar_core::{Calendar, Error};

#[tokio::test]
async fn ics_goes_in_and_out_by_media_type() {
    let r = repo().await;
    let report = r.import_text("text/calendar", ICS).await.unwrap();
    assert_eq!(report.created.len(), 2);
    let out = r
        .export_text(&report.created, "text/calendar")
        .await
        .unwrap();
    assert!(out.contains("SUMMARY:Jour fixe") && out.contains("SUMMARY:Second"));
}

#[tokio::test]
async fn jscalendar_goes_in_and_out_by_media_type() {
    let r = repo().await;
    let ics = r.import_text("text/calendar", ICS).await.unwrap();
    let one = r
        .export_text(&ics.created[..1], "application/jscalendar+json")
        .await
        .unwrap();
    assert!(one.starts_with('{') && one.contains("\"@type\":\"Event\""));
    let all = r
        .export_text(&ics.created, "Application/JSCalendar+JSON; charset=utf-8")
        .await
        .unwrap();
    assert!(all.starts_with('['));

    // Into a second calendar: both the object and the array read back as the same events.
    for text in [&one, &all] {
        let other = repo().await;
        let back = other
            .import_text("application/jscalendar+json", text)
            .await
            .unwrap();
        assert!(back.failed.is_empty(), "{:?}", back.failed);
        assert_eq!(back.created.len(), if text == &one { 1 } else { 2 });
    }
}

#[tokio::test]
async fn an_unknown_media_type_is_an_error() {
    let r = repo().await;
    assert!(matches!(
        r.import_text("text/plain", ICS).await,
        Err(Error::MediaType(m)) if m == "text/plain"
    ));
    assert!(matches!(
        r.export_text(&[], "application/xml").await,
        Err(Error::MediaType(_))
    ));
    assert!(r.import_text("", "").await.is_err());
    assert_eq!(
        r.list_events().await.unwrap().len(),
        0,
        "nothing was stored"
    );
}
