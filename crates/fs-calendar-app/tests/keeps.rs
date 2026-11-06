//! Editing keeps everything else on the event, through the real core and the real state machine.
//!
//! Pattern: Round-trip tests — the event as JSON before and after, compared but for what was edited.

mod common;

use common::{World, d, json_of, rich};
use fs_components_workbenches::calendar::{CalendarState, Shown};
use serde_json::json;

fn open_event(w: &World, key: &str) -> CalendarState {
    let mut s = w.state();
    assert!(matches!(
        s.select(key),
        fs_components_workbenches::calendar::Outcome::Done
    ));
    assert!(matches!(s.shown(), Shown::Item { .. }), "{:?}", s.shown());
    s
}

fn saved(s: &CalendarState) {
    assert_eq!(
        s.editing().map(|e| e.error.clone()),
        None,
        "{:?}",
        s.notice()
    );
}

#[test]
fn editing_the_title_changes_the_title_and_nothing_else() {
    let w = World::berlin();
    let key = w.add_json(rich());
    let before = json_of(&w, &key);
    let mut s = open_event(&w, &key);
    s.begin_edit();
    s.set_field("title", "Kick-off 2");
    s.save();
    saved(&s);
    let mut expected = before;
    expected["title"] = json!("Kick-off 2");
    assert_eq!(json_of(&w, &key), expected);
}

#[test]
fn saving_description_location_or_zone_touches_only_that() {
    let w = World::berlin();
    let key = w.add_json(rich());
    let mut s = open_event(&w, &key);
    let mut expected = json_of(&w, &key);

    s.begin_edit();
    s.set_field("description", "Only this");
    s.save();
    saved(&s);
    expected["description"] = json!("Only this");
    assert_eq!(json_of(&w, &key), expected);

    s.begin_edit();
    s.set_field("location", "Other hall");
    s.save();
    saved(&s);
    expected["locations"]["l1"]["name"] = json!("Other hall");
    assert_eq!(
        json_of(&w, &key),
        expected,
        "the first location, its other corners stay"
    );

    s.begin_edit();
    s.set_field("location", "");
    s.save();
    saved(&s);
    expected["locations"]["l1"]
        .as_object_mut()
        .unwrap()
        .remove("name");
    assert_eq!(
        json_of(&w, &key),
        expected,
        "an emptied name takes only the name"
    );

    s.begin_edit();
    s.set_field("time_zone", "Europe/Paris");
    s.save();
    saved(&s);
    expected["timeZone"] = json!("Europe/Paris");
    expected["sequence"] = json!(8);
    assert_eq!(
        json_of(&w, &key),
        expected,
        "same clock times; the core counts the sequence"
    );
}

#[test]
fn moving_an_event_writes_start_and_duration_and_the_rest_stays() {
    let w = World::berlin();
    let key = w.add_json(rich());
    let mut expected = json_of(&w, &key);
    let mut s = open_event(&w, &key);
    s.begin_edit();
    s.set_field("start", "2026-11-25 09:00");
    s.set_field("end", "2026-11-25 09:45");
    s.save();
    saved(&s);
    expected["start"] = json!("2026-11-25T09:00:00");
    expected["duration"] = json!("PT45M");
    expected["sequence"] = json!(8);
    assert_eq!(json_of(&w, &key), expected);
    assert_eq!(
        s.anchor(),
        d("2026-11-06"),
        "the 25th is in the month shown"
    );
}

#[test]
fn saving_nothing_writes_nothing() {
    let w = World::berlin();
    let key = w.add_json(rich());
    let before = w.stored(&key);
    let mut s = open_event(&w, &key);
    s.begin_edit();
    s.save();
    assert_eq!(w.stored(&key).envelope.updated, before.envelope.updated);
    assert_eq!(w.stored(&key).content, before.content);
}

const ICS: &str = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//x//y//EN\r\nBEGIN:VEVENT\r\nUID:ev-1\r\nDTSTAMP:20261101T000000Z\r\nDTSTART;TZID=Europe/Berlin:20261124T100000\r\nDTEND;TZID=Europe/Berlin:20261124T113000\r\nSUMMARY:Jour fixe\r\nDESCRIPTION:Hello\\, world\r\nLOCATION:Berlin\r\nORGANIZER;CN=Me:mailto:me@example.org\r\nATTENDEE;CN=Ben;PARTSTAT=NEEDS-ACTION;RSVP=TRUE:mailto:ben@example.org\r\nSTATUS:CONFIRMED\r\nX-VENDOR-THING:kept\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";

#[test]
fn the_original_of_an_import_and_its_participants_survive_an_edit() {
    let w = World::berlin();
    let report = w.runtime.block_on(w.repo.import_ics(ICS)).unwrap();
    let key = report.created[0].to_string();
    let before = w.stored(&key);
    let original = before
        .original
        .clone()
        .expect("an imported event has its original");
    assert!(before.content.to_string().contains("ben@example.org"));
    let mut s = open_event(&w, &key);
    s.begin_edit();
    s.set_field("title", "Renamed");
    s.save();
    saved(&s);
    let after = w.stored(&key);
    assert_eq!(after.original, Some(original), "byte for byte");
    let mut expected = before.content.clone();
    expected["title"] = json!("Renamed");
    let strip = |v: &mut serde_json::Value| {
        v.as_object_mut().unwrap().remove("updated");
    };
    let mut got = after.content.clone();
    strip(&mut expected);
    strip(&mut got);
    assert_eq!(got, expected);
}
