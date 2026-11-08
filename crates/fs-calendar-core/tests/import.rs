//! JSCalendar import (2.0 and upgraded 1.0).

mod common;

use common::{DEMO, repo, t};
use fs_calendar_core::*;
use serde_json::json;

#[tokio::test]
async fn import_demo_events() {
    let r = repo().await;
    let rep = r
        .import_jscalendar(&std::fs::read_to_string(DEMO).unwrap())
        .await
        .unwrap();
    assert_eq!(
        (rep.created.len(), rep.skipped.len(), rep.failed.len()),
        (5, 0, 0),
        "{rep:?}"
    );
    assert_eq!(rep.notes, [] as [std::string::String; 0]);
    let again = r
        .import_jscalendar(&std::fs::read_to_string(DEMO).unwrap())
        .await
        .unwrap();
    assert_eq!((again.created.len(), again.skipped.len()), (0, 5));
    let next = r.upcoming(t("2026-11-05T00:00:00Z"), 30).await.unwrap();
    assert_eq!(next.len(), 4, "the fifth draft is on 8 December");
    let next = r.upcoming(t("2026-11-05T00:00:00Z"), 40).await.unwrap();
    assert_eq!(next.len(), 5);
    let titles: Vec<_> = next
        .iter()
        .map(|i| i.occurrence.event.display_title())
        .collect();
    assert!(
        titles[0].contains("NLnet") && titles[4].contains("Open Source Business Alliance"),
        "{titles:?}"
    );
    assert_eq!(next[0].occurrence.start_utc, t("2026-11-24T09:00:00Z"));
    // only the first three fall into 20 days
    assert_eq!(
        r.upcoming(t("2026-11-05T00:00:00Z"), 22)
            .await
            .unwrap()
            .len(),
        2
    );
    // accept: the organisation replies
    let first = next[0].object;
    r.respond(first, "webcontact@nlnet.nl", Response::Accept, None)
        .await
        .unwrap();
    let ev = r.get_event(first).await.unwrap().unwrap();
    assert_eq!(
        ev.participants["p1"].participation_status.as_deref(),
        Some("accepted")
    );
    assert_eq!(
        ev.participants["owner"].participation_status.as_deref(),
        Some("accepted")
    );
}

#[tokio::test]
async fn import_rfc8984_data_upgrades() {
    let r = repo().await;
    let v1 = json!([{"@type":"Event","uid":"old-1","title":"Jour fixe","start":"2026-11-03T10:00:00","timeZone":"Europe/Berlin","duration":"PT30M",
        "recurrenceRules":[{"frequency":"weekly","count":2}]}]);
    let rep = r.import_jscalendar(&v1.to_string()).await.unwrap();
    assert_eq!(rep.created.len(), 1, "{rep:?}");
    assert_ne!(rep.notes, [] as [std::string::String; 0]);
    let e = r.get_event(rep.created[0]).await.unwrap().unwrap();
    assert_eq!(e.version.as_deref(), Some("2.0"));
    assert_eq!(e.recurrence_rule.unwrap().count, Some(2));
}
