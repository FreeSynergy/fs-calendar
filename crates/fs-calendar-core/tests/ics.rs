//! iCalendar import / export, original keeping (ADR 0030).

mod common;

use common::{DEMO, ICS, repo, t};
use fs_calendar_core::*;

#[test]
fn ics_is_split_per_uid_with_exact_text() {
    let parts = ical::split_ics(ICS);
    assert_eq!(
        parts.iter().map(|p| p.uid.as_str()).collect::<Vec<_>>(),
        ["ev-1", "ev-2", "todo-1"]
    );
    for p in &parts {
        assert!(
            p.text
                .starts_with("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//x//y//EN\r\nBEGIN:V"),
            "{}",
            p.text
        );
        assert!(p.text.ends_with("END:VCALENDAR\r\n"));
        assert_eq!(
            p.text.matches("BEGIN:VEVENT").count() + p.text.matches("BEGIN:VTODO").count(),
            1
        );
        assert!(
            ICS.contains(p.text.lines().nth(4).unwrap()),
            "lines are copied unchanged"
        );
    }
    // a single-UID file is kept whole
    let single = ical::split_ics(&parts[1].text);
    assert_eq!(single.len(), 1);
    assert_eq!(single[0].text, parts[1].text);
    assert!(ical::split_ics("nonsense").is_empty());
}

#[tokio::test]
async fn ics_import_keeps_original_and_converts() {
    let r = repo().await;
    let rep = r.import_ics(ICS).await.unwrap();
    assert_eq!(
        (rep.created.len(), rep.skipped.len(), rep.failed.len()),
        (3, 0, 0),
        "{rep:?}"
    );
    let ev = r.get(rep.created[0]).await.unwrap().unwrap();
    let orig = ev.original.as_ref().unwrap();
    assert_eq!(orig.media_type, "text/calendar");
    let text = String::from_utf8(orig.bytes.clone()).unwrap();
    assert!(text.contains("UID:ev-1") && !text.contains("UID:ev-2"));
    assert_eq!(
        ev.index["tzdb_version"],
        [fs_calendar_core::time::tzdb_version()]
    );
    assert_eq!(ev.envelope.name, "Jour fixe");
    let e = Event::from_value(ev.content.clone()).unwrap();
    assert_eq!(e.version.as_deref(), Some("2.0"));
    assert_eq!(e.start.as_deref(), Some("2026-11-24T10:00:00"));
    assert_eq!(e.time_zone.as_deref(), Some("Europe/Berlin"));
    assert_eq!(e.duration.as_deref(), Some("PT1H30M"));
    let rule = e.recurrence_rule.as_ref().unwrap();
    assert_eq!((rule.frequency.as_str(), rule.count), ("weekly", Some(4)));
    assert!(
        e.recurrence_overrides.contains_key("2026-12-01T10:00:00"),
        "EXDATE -> excluded override"
    );
    assert_eq!(e.participants.len(), 2);
    // expansion: 24 Nov, (1 Dec excluded), 8 Dec, 15 Dec
    let occ = r
        .occurrences(t("2026-11-01T00:00:00Z"), t("2027-01-01T00:00:00Z"))
        .await
        .unwrap();
    let starts: Vec<_> = occ
        .iter()
        .filter(|i| i.object == rep.created[0])
        .map(|i| i.occurrence.recurrence_id.clone())
        .collect();
    assert_eq!(
        starts,
        [
            "2026-11-24T10:00:00",
            "2026-12-08T10:00:00",
            "2026-12-15T10:00:00"
        ]
    );
    // the task
    let task = r.get(rep.created[2]).await.unwrap().unwrap();
    assert_eq!(task.envelope.object_type, "Task");
    assert_eq!(task.index["due_utc"], ["2026-11-12T17:00:00Z"]);
    // re-import skips
    let again = r.import_ics(ICS).await.unwrap();
    assert_eq!((again.created.len(), again.skipped.len()), (0, 3));
    // garbage
    let bad = r
        .import_ics("BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:x\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n")
        .await
        .unwrap();
    assert_eq!(
        bad.created.len() + bad.failed.len(),
        1,
        "no panic: imported or reported"
    );
}

#[tokio::test]
async fn ics_export_roundtrip() {
    let r = repo().await;
    let rep = r.import_ics(ICS).await.unwrap();
    let out = r.export_ics(&rep.created).await.unwrap();
    for needle in [
        "BEGIN:VCALENDAR",
        "BEGIN:VEVENT",
        "BEGIN:VTODO",
        "UID:ev-1",
        "SUMMARY:Jour fixe",
        "RRULE:FREQ=WEEKLY;COUNT=4;BYDAY=TU",
        "DTSTART;TZID=Europe/Berlin:20261124T100000",
        "EXDATE;TZID=Europe/Berlin:20261201T100000",
        "mailto:ben@example.org",
        "DUE:20261112T170000Z",
    ] {
        assert!(out.contains(needle), "{needle} in\n{out}");
    }
    // re-import the export into a fresh calendar: same structure
    let r2 = repo().await;
    let rep2 = r2.import_ics(&out).await.unwrap();
    assert_eq!(rep2.created.len(), 3, "{rep2:?}\n{out}");
    let a = r.get_event(rep.created[0]).await.unwrap().unwrap();
    let b = r2
        .list_events()
        .await
        .unwrap()
        .into_iter()
        .find_map(|o| {
            Event::from_value(o.content)
                .ok()
                .filter(|e| e.uid == "ev-1")
        })
        .unwrap();
    assert_eq!(
        (a.start, a.time_zone, a.duration, a.recurrence_rule),
        (b.start, b.time_zone, b.duration, b.recurrence_rule)
    );
    // a native JSCalendar event exports too
    let mut e = Event::new("Native", "2026-11-24T10:00:00", "Europe/Berlin", "PT1H");
    e.add_invitee("Ben", "ben@example.org");
    let id = r.create(e).await.unwrap().envelope.id;
    let out = r.export_ics(&[id]).await.unwrap();
    assert!(
        out.contains("SUMMARY:Native")
            && out.contains("DTSTART;TZID=Europe/Berlin:20261124T100000"),
        "{out}"
    );
}

#[tokio::test]
async fn demo_events_export_as_ics() {
    let r = repo().await;
    let rep = r
        .import_jscalendar(&std::fs::read_to_string(DEMO).unwrap())
        .await
        .unwrap();
    let out = r.export_ics(&rep.created).await.unwrap();
    assert_eq!(out.matches("BEGIN:VEVENT").count(), 5);
    assert!(
        out.contains("mailto:webcontact@nlnet.nl")
            && out.contains("DTSTART;TZID=Europe/Berlin:20261124T100000")
    );
}
