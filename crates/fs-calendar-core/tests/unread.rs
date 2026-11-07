//! An import says what it could not read, and nothing it did read.

mod common;

use common::repo;
use fs_calendar_core::Calendar;

fn ics(body: &str) -> String {
    format!(
        "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:x\r\nBEGIN:VEVENT\r\nUID:u1\r\nDTSTAMP:20261101T000000Z\r\nSUMMARY:S\r\n{body}END:VEVENT\r\nEND:VCALENDAR\r\n"
    )
}

async fn notes(body: &str) -> (usize, Vec<String>, Vec<String>) {
    let report = repo()
        .await
        .import_text("text/calendar", &ics(body))
        .await
        .unwrap();
    (report.created.len(), report.notes, report.failed)
}

#[tokio::test]
async fn garbage_in_the_end_the_length_or_the_rule_is_said() {
    for (body, name) in [
        ("DTSTART:20261201T090000Z\r\nDTEND:garbage\r\n", "DTEND"),
        (
            "DTSTART:20261201T090000Z\r\nDURATION:garbage\r\n",
            "DURATION",
        ),
        (
            "DTSTART:20261201T090000Z\r\nDURATION:PT1H\r\nRRULE:FREQ=SOMETIMES\r\n",
            "RRULE",
        ),
    ] {
        let (created, notes, failed) = notes(body).await;
        let said = notes
            .iter()
            .chain(&failed)
            .any(|n| n.starts_with("u1") && n.contains(name));
        assert!(said, "{name}: created {created}, {notes:?}, {failed:?}");
    }
}

#[tokio::test]
async fn garbage_in_the_start_is_said_or_refused() {
    let (created, notes, failed) = notes("DTSTART:garbage\r\nDURATION:PT1H\r\n").await;
    assert_eq!(created, 0, "an event without a start is not stored");
    assert!(
        notes.iter().chain(&failed).any(|n| n.contains("u1")),
        "{notes:?} {failed:?}"
    );
}

#[tokio::test]
async fn what_was_read_gives_no_note_and_an_alarm_or_a_zone_is_no_false_alarm() {
    let clean = "DTSTART;TZID=Europe/Berlin:20261201T100000\r\nDTEND;TZID=Europe/Berlin:20261201T110000\r\nRRULE:FREQ=WEEKLY;COUNT=2\r\nBEGIN:VALARM\r\nACTION:DISPLAY\r\nTRIGGER:-PT15M\r\nDURATION:PT5M\r\nREPEAT:2\r\nDESCRIPTION:x\r\nEND:VALARM\r\n";
    let (created, notes, failed) = notes(clean).await;
    assert_eq!(created, 1, "{failed:?}");
    assert!(
        notes.iter().all(|n| !n.contains("could not be read")),
        "{notes:?}"
    );
    let folded = "DTSTART:20261201T090000Z\r\nDESCRIPTION:a long text\r\n  DTEND:garbage\r\nDTEND:20261201T100000Z\r\n";
    let (created, notes, _) = self::notes(folded).await;
    assert_eq!(created, 1);
    assert!(
        notes.iter().all(|n| !n.contains("could not be read")),
        "a continuation line is no property: {notes:?}"
    );
}
