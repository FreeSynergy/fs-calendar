//! What an import says about what it did: counts, names and what it never names. An event whose zone
//! no zone answers to is refused by the core, by its uid and the name of the zone (it is not stored).
//!
//! Pattern: Integration tests over the real core; files in a scratch folder.

mod common;

use common::{Scratch, World};
use fs_calendar_app::{ImportSummary, import_file};

fn event(body: &str) -> String {
    format!("BEGIN:VEVENT\r\nDTSTAMP:20261101T000000Z\r\n{body}\r\nEND:VEVENT\r\n")
}

fn file(events: &[String]) -> String {
    format!(
        "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//x//y//EN\r\n{}END:VCALENDAR\r\n",
        events.concat()
    )
}

fn import(w: &World, text: &str) -> ImportSummary {
    let dir = Scratch::new();
    let path = dir.file("in.ics");
    std::fs::write(&path, text).unwrap();
    import_file(w.repo.as_ref(), w.runtime.handle(), &path).unwrap()
}

const FINE: &str =
    "UID:fine-1\r\nDTSTART;TZID=Europe/Berlin:20261124T100000\r\nDURATION:PT1H\r\nSUMMARY:Fine";

#[test]
fn a_length_too_long_costs_its_event_by_uid_and_those_after_it_arrive() {
    let w = World::berlin();
    let summary = import(
        &w,
        &file(&[
            event(FINE),
            event(
                "UID:long-1\r\nDTSTART:20261130T090000Z\r\nDURATION:P99999999999D\r\nSUMMARY:Long",
            ),
            event("UID:after-1\r\nDTSTART:20261201T090000Z\r\nDURATION:PT1H\r\nSUMMARY:After"),
        ]),
    );
    assert_eq!(summary.created, 2, "{}", summary.message());
    assert_eq!(summary.failed.len(), 1, "{}", summary.message());
    assert!(
        summary.failed[0].starts_with("long-1:"),
        "{}",
        summary.message()
    );
    assert!(
        summary.message().contains("Imported 2"),
        "{}",
        summary.message()
    );
    assert_eq!(w.item("2026-12-01", "After").start.clock(), "10:00");
}

#[test]
fn an_unknown_zone_is_found_however_the_file_writes_it() {
    let cases = [
        (
            "lowercase",
            "uid:low-1\r\ndtstart;tzid=Not/AZone:20261124T100000\r\nduration:PT1H\r\nsummary:Lowercase",
            "low-1",
        ),
        (
            "params",
            "UID;X-THING=1:par-1\r\nDTSTART;TZID=Not/AZone:20261124T100000\r\nDURATION:PT1H\r\nSUMMARY;LANGUAGE=de:Mit Sprache",
            "par-1",
        ),
        (
            "escapes",
            "UID:esc-1\r\nDTSTART;TZID=Not/AZone:20261124T100000\r\nDURATION:PT1H\r\nSUMMARY:One\\, two",
            "esc-1",
        ),
        (
            "end only",
            "UID:end-1\r\nDTSTART;TZID=Europe/Berlin:20261124T100000\r\nDTEND;TZID=Not/AZone:20261124T110000\r\nSUMMARY:End only",
            "end-1",
        ),
    ];
    for (what, body, named) in cases {
        let w = World::berlin();
        let summary = import(&w, &file(&[event(body)]));
        assert_eq!(summary.created, 0, "{what}: {}", summary.message());
        assert_eq!(summary.failed.len(), 1, "{what}: {}", summary.message());
        assert!(
            summary.failed[0].starts_with(&format!("{named}:"))
                && summary.failed[0].contains("Not/AZone"),
            "{what}: {}",
            summary.message()
        );
        assert!(!summary.is_clean(), "{what}");
    }
}

#[test]
fn an_unknown_zone_only_on_an_override_is_found() {
    let w = World::berlin();
    let master = event(
        "UID:rec-1\r\nDTSTART;TZID=Europe/Berlin:20261124T100000\r\nDURATION:PT1H\r\nRRULE:FREQ=WEEKLY;COUNT=3\r\nSUMMARY:Weekly",
    );
    let moved = event(
        "UID:rec-1\r\nRECURRENCE-ID;TZID=Europe/Berlin:20261201T100000\r\nDTSTART;TZID=Not/AZone:20261201T120000\r\nDURATION:PT1H\r\nSUMMARY:Moved",
    );
    let summary = import(&w, &file(&[master, moved]));
    assert_eq!(summary.created, 0, "{}", summary.message());
    assert!(
        summary.failed[0].starts_with("rec-1:") && summary.failed[0].contains("Not/AZone"),
        "{}",
        summary.message()
    );
}

#[test]
fn the_name_is_the_events_not_its_alarms_and_what_was_not_touched_is_not_named() {
    let w = World::berlin();
    let with_alarm = event(
        "UID:ala-1\r\nDTSTART;TZID=Not/AZone:20261124T100000\r\nDURATION:PT1H\r\nSUMMARY:The event\r\nBEGIN:VALARM\r\nACTION:EMAIL\r\nTRIGGER:-PT15M\r\nSUMMARY:The alarm\r\nDESCRIPTION:x\r\nATTENDEE:mailto:a@x.org\r\nEND:VALARM",
    );
    let text = file(&[with_alarm]);
    let first = import(&w, &text);
    assert_eq!(first.failed.len(), 1, "{}", first.message());
    assert!(first.failed[0].starts_with("ala-1:"), "{}", first.message());
    assert!(
        !first.failed[0].contains("alarm"),
        "the event is named, not its alarm: {}",
        first.message()
    );
}

#[test]
fn a_known_zone_and_a_name_the_parser_maps_are_no_loss() {
    let w = World::berlin();
    let mapped = event(
        "UID:win-1\r\nDTSTART;TZID=W. Europe Standard Time:20261124T100000\r\nDTEND;TZID=W. Europe Standard Time:20261124T110000\r\nSUMMARY:Windows name",
    );
    let summary = import(&w, &file(&[event(FINE), mapped]));
    assert!(summary.zone_lost.is_empty(), "{}", summary.message());
    eprintln!("stored zone: {}", w.stored_uid("win-1")["timeZone"]);
}

#[test]
fn a_marked_event_that_lost_its_zone_is_not_called_by_its_title() {
    let w = World::berlin();
    let secret = event(
        "UID:sec-1\r\nDTSTART;TZID=Not/AZone:20261124T100000\r\nDURATION:PT1H\r\nSUMMARY:Clinic\r\nCLASS:PRIVATE",
    );
    let summary = import(&w, &file(&[secret]));
    assert_eq!(summary.failed.len(), 1, "{}", summary.message());
    assert!(
        summary.failed[0].starts_with("sec-1:"),
        "{}",
        summary.message()
    );
    assert!(
        !summary.message().contains("Clinic"),
        "{}",
        summary.message()
    );
}

#[test]
fn tasks_still_go_to_the_task_program_one_by_one() {
    let w = World::berlin();
    let todo = "BEGIN:VTODO\r\nUID:todo-1\r\nDTSTAMP:20261101T000000Z\r\nDUE:20261112T170000Z\r\nSUMMARY:Minutes\r\nEND:VTODO\r\n";
    let summary = import(&w, &file(&[event(FINE), todo.to_string()]));
    assert_eq!(summary.created, 2, "{}", summary.message());
    assert_eq!(w.item("2026-11-12", "Minutes").start.clock(), "18:00");
}
