//! `importText` and `exportText`: both media types, the report, the problems.

mod common;

use common::{items_of, set_id, with_server};
use fs_interfaces::events_capnp::items;
use fs_interfaces::rpc::{Problem, read_uuid};

const INVALID: &str = "https://freesynergy.net/problems/calendar/invalid";

fn ics(events: &[&str]) -> String {
    use std::fmt::Write;
    let body = events.iter().fold(String::new(), |mut body, b| {
        write!(
            body,
            "BEGIN:VEVENT\r\nDTSTAMP:20261101T000000Z\r\n{b}\r\nEND:VEVENT\r\n"
        )
        .unwrap();
        body
    });
    format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:x\r\n{body}END:VCALENDAR\r\n")
}

fn texts(list: capnp::text_list::Reader<'_>) -> Vec<String> {
    list.iter()
        .map(|t| t.unwrap().to_string().unwrap())
        .collect()
}

async fn import(
    items: &items::Client,
    media: &str,
    text: &str,
) -> capnp::Result<(
    usize,
    Vec<String>,
    Vec<String>,
    Vec<String>,
    Vec<uuid::Uuid>,
)> {
    let mut req = items.import_text_request();
    req.get().set_media_type(media);
    req.get().set_text(text);
    let answer = req.send().promise.await?;
    let report = answer.get()?.get_report()?;
    let created: Vec<_> = report
        .get_created()?
        .iter()
        .map(|u| read_uuid(u).unwrap())
        .collect();
    Ok((
        created.len(),
        texts(report.get_skipped()?),
        texts(report.get_failed()?),
        texts(report.get_notes()?),
        created,
    ))
}

#[tokio::test(flavor = "current_thread")]
async fn an_ics_import_reports_what_was_created_skipped_failed_and_noted() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        let file = ics(&[
            "UID:a1\r\nDTSTART:20261201T090000Z\r\nSUMMARY:A",
            "UID:z1\r\nDTSTART;TZID=Not/AZone:20261201T090000\r\nSUMMARY:Lost zone",
        ])
        .replace(
            "END:VCALENDAR",
            "BEGIN:VTODO\r\nUID:t1\r\nDTSTAMP:20261101T000000Z\r\nSUMMARY:T\r\nEND:VTODO\r\nEND:VCALENDAR",
        );
        let (n, skipped, failed, notes, _) = import(&items, "text/calendar; charset=utf-8", &file).await.unwrap();
        assert_eq!(n, 1);
        assert_eq!(skipped, [] as [std::string::String; 0]);
        assert_eq!(failed.len(), 1, "{failed:?}");
        assert!(failed[0].starts_with("z1:") && failed[0].contains("Not/AZone"), "{failed:?}");
        assert!(notes.iter().any(|n| n.contains("t1") && n.contains("no task program")), "{notes:?}");
        let (n, skipped, _, _, _) = import(&items, "text/calendar", &file).await.unwrap();
        assert_eq!((n, skipped), (0, vec!["a1".to_string()]));
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn jscalendar_goes_in_and_both_formats_come_out() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        let text = format!(
            "[{},{}]",
            common::json("j1", "One", "2026-12-01T10:00:00"),
            common::json("j2", "Two", "2026-12-02T10:00:00")
        );
        let (n, _, failed, _, created) = import(&items, "application/jscalendar+json", &text)
            .await
            .unwrap();
        assert_eq!((n, failed.len()), (2, 0), "{failed:?}");

        for (media, needle) in [
            ("text/calendar", "SUMMARY:One"),
            ("application/jscalendar+json", "\"title\":\"One\""),
        ] {
            let mut req = items.export_text_request();
            req.get().set_media_type(media);
            let mut ids = req.get().init_ids(1);
            set_id(ids.reborrow().get(0), created[0]);
            let answer = req.send().promise.await.unwrap();
            let out = answer
                .get()
                .unwrap()
                .get_text()
                .unwrap()
                .to_string()
                .unwrap();
            assert!(out.contains(needle), "{media}: {out}");
            assert!(!out.contains("Two"), "{media}: only what was asked for");
        }
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn an_unknown_media_type_is_a_typed_problem_and_stores_nothing() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        let error = import(&items, "application/pdf", "x").await.err().unwrap();
        let problem = Problem::from_error(&error).unwrap();
        assert_eq!(
            (problem.problem_type.as_str(), problem.status),
            (INVALID, 422)
        );
        let mut req = items.export_text_request();
        req.get().set_media_type("application/pdf");
        let error = req.send().promise.await.err().unwrap();
        assert_eq!(Problem::from_error(&error).unwrap().problem_type, INVALID);
        let list = items.list_request().send().promise.await.unwrap();
        assert_eq!(
            list.get()
                .unwrap()
                .get_page()
                .unwrap()
                .get_items()
                .unwrap()
                .len(),
            0
        );
    })
    .await;
}
