//! iCalendar files in and out.
//!
//! Pattern: Integration tests over the real core; files in a scratch folder.

mod common;

use common::{Scratch, World, rich};
use fs_calendar_app::{export_all, import_file};
use serde_json::json;

const TWO: &str = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//x//y//EN\r\nBEGIN:VEVENT\r\nUID:ev-1\r\nDTSTAMP:20261101T000000Z\r\nDTSTART;TZID=Europe/Berlin:20261124T100000\r\nDTEND;TZID=Europe/Berlin:20261124T113000\r\nSUMMARY:First\r\nRRULE:FREQ=WEEKLY;COUNT=4;BYDAY=TU\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nUID:ev-2\r\nDTSTAMP:20261101T000000Z\r\nDTSTART:20261130T090000Z\r\nDURATION:PT2H\r\nSUMMARY:Second\r\nEND:VEVENT\r\nBEGIN:VTODO\r\nUID:todo-1\r\nDTSTAMP:20261101T000000Z\r\nDUE:20261112T170000Z\r\nSUMMARY:Minutes\r\nEND:VTODO\r\nEND:VCALENDAR\r\n";

fn export(
    w: &World,
    path: &std::path::Path,
    released: &[String],
) -> Result<fs_calendar_app::ExportReport, String> {
    export_all(w.repo.as_ref(), w.runtime.handle(), path, released)
}

#[test]
fn a_file_is_imported_known_events_are_skipped_and_series_appear() {
    let (w, dir) = (World::berlin(), Scratch::new());
    let file = dir.file("two.ics");
    std::fs::write(&file, TWO).unwrap();
    let first = import_file(w.repo.as_ref(), w.runtime.handle(), &file).unwrap();
    assert_eq!(
        first.created,
        3,
        "two events and a task: {}",
        first.message()
    );
    assert!(first.is_clean(), "{}", first.message());
    let again = import_file(w.repo.as_ref(), w.runtime.handle(), &file).unwrap();
    assert_eq!(
        (again.created, again.skipped),
        (0, 3),
        "{}",
        again.message()
    );
    assert_eq!(
        w.items("2026-12-01", "2026-12-01").len(),
        1,
        "the second of the series"
    );
    assert_eq!(w.item("2026-11-30", "Second").start.clock(), "10:00");
}

#[test]
fn a_missing_file_and_a_file_without_a_calendar_are_errors() {
    let (w, dir) = (World::berlin(), Scratch::new());
    let missing = import_file(w.repo.as_ref(), w.runtime.handle(), &dir.file("none.ics"));
    assert!(missing.unwrap_err().contains("cannot read"));
    let file = dir.file("text.ics");
    std::fs::write(&file, "just text").unwrap();
    let empty = import_file(w.repo.as_ref(), w.runtime.handle(), &file);
    assert!(empty.unwrap_err().contains("no calendar"));
}

#[test]
fn a_broken_event_is_named_and_the_others_arrive() {
    let (w, dir) = (World::berlin(), Scratch::new());
    let file = dir.file("mixed.ics");
    let broken = TWO.replace("DTSTART:20261130T090000Z", "DTSTART:the day after tomorrow");
    std::fs::write(&file, broken).unwrap();
    let summary = import_file(w.repo.as_ref(), w.runtime.handle(), &file).unwrap();
    assert_eq!(summary.created, 2, "{}", summary.message());
    assert_eq!(summary.failed.len(), 1, "{}", summary.message());
    assert!(summary.failed[0].contains("ev-2"), "{}", summary.message());
    assert!(!summary.is_clean());
}

#[test]
fn an_export_reads_back_and_a_new_store_holds_the_same_events() {
    let (w, dir) = (World::berlin(), Scratch::new());
    let file = dir.file("two.ics");
    std::fs::write(&file, TWO).unwrap();
    import_file(w.repo.as_ref(), w.runtime.handle(), &file).unwrap();
    let out = dir.file("out.ics");
    let report = export(&w, &out, &[]).unwrap();
    assert_eq!(report.written, 2);
    let text = std::fs::read_to_string(&out).unwrap();
    assert!(
        text.contains("SUMMARY:First") && text.contains("RRULE:"),
        "{text}"
    );
    assert!(
        !text.contains("VTODO"),
        "tasks are not the calendar's to export"
    );
    let other = World::berlin();
    let back = import_file(other.repo.as_ref(), other.runtime.handle(), &out).unwrap();
    assert_eq!(back.created, 2, "{}", back.message());
    assert_eq!(other.item("2026-11-24", "First").start.clock(), "10:00");
    assert_eq!(
        other.items("2026-12-15", "2026-12-15").len(),
        1,
        "fourth of four"
    );
    assert!(other.items("2026-12-22", "2026-12-22").is_empty());
}

#[test]
fn an_export_never_replaces_a_file_and_leaves_no_temporary_file() {
    let (w, dir) = (World::berlin(), Scratch::new());
    w.add_json(rich());
    let out = dir.file("kept.ics");
    std::fs::write(&out, "mine").unwrap();
    assert!(export(&w, &out, &[]).is_err());
    assert_eq!(std::fs::read_to_string(&out).unwrap(), "mine");
    let names: Vec<_> = std::fs::read_dir(dir.file("")).unwrap().collect();
    assert_eq!(names.len(), 1, "{names:?}");
}

#[test]
fn an_empty_calendar_exports_without_a_panic() {
    let (w, dir) = (World::berlin(), Scratch::new());
    let out = dir.file("empty.ics");
    let report = export(&w, &out, &[]).unwrap();
    assert_eq!(report.written, 0);
    eprintln!("{:?}", std::fs::read_to_string(&out).unwrap());
}

#[test]
fn an_all_day_and_a_floating_event_come_back_the_same_from_a_file() {
    let (w, dir) = (World::berlin(), Scratch::new());
    let mut s = w.state();
    s.begin_create();
    s.set_field("title", "Whole day");
    s.toggle_field("all_day");
    s.set_field("end", "2026-11-08 10:00");
    s.save();
    assert!(s.editing().is_none(), "{:?}", s.notice());
    let key = s.selected().unwrap().to_string();
    let stored = w.stored(&key).content;
    assert_eq!(stored["showWithoutTime"], json!(true));
    assert_eq!(stored["duration"], json!("P3D"));
    assert_eq!(stored.get("timeZone"), None, "a day has no zone");
    s.begin_create();
    s.set_field("title", "Clock only");
    s.set_field("time_zone", "");
    s.save();
    assert!(s.editing().is_none(), "{:?}", s.notice());
    let out = dir.file("out.ics");
    export(&w, &out, &[]).unwrap();
    let text = std::fs::read_to_string(&out).unwrap();
    assert!(text.contains("VALUE=DATE"), "{text}");
    let other = World::shown_in("Asia/Tokyo");
    let back = import_file(other.repo.as_ref(), other.runtime.handle(), &out).unwrap();
    assert_eq!(back.created, 2, "{}", back.message());
    let day = other.item("2026-11-07", "Whole day");
    assert!(day.all_day);
    assert_eq!(day.start.day, common::d("2026-11-06"));
    assert_eq!(
        other.item("2026-11-06", "Clock only").start.clock(),
        "09:00"
    );
}
