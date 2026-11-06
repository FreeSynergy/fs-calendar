//! Creating, editing and deleting through the real core.
//!
//! Pattern: State machine tests over the real adapter.

mod common;

use common::{World, d, json_of, rich, sheet_text};
use fs_calendar_core::Event;
use fs_components_workbenches::calendar::{CalendarSource, Picked, Shown};
use serde_json::json;

#[test]
fn a_new_event_is_made_on_the_day_chosen_in_the_zone_shown_and_shows_at_once() {
    let w = World::berlin();
    let mut s = w.state();
    s.pick(&Picked::Day(d("2026-11-11")));
    s.begin_create();
    let draft = &s.editing().unwrap().draft;
    assert_eq!(draft.value("start"), Some("2026-11-11 09:00"));
    assert_eq!(draft.value("time_zone"), Some("Europe/Berlin"));
    s.set_field("title", "Dentist");
    s.set_field("location", "Hauptstr. 1");
    s.set_field("description", "Bring the card\nand the form");
    s.save();
    assert!(s.editing().is_none(), "{:?}", s.notice());
    let key = s.selected().unwrap().to_string();
    let stored = w.stored(&key).content;
    assert_eq!(stored["start"], json!("2026-11-11T09:00:00"));
    assert_eq!(stored["duration"], json!("PT1H"));
    assert_eq!(stored["timeZone"], json!("Europe/Berlin"));
    assert_eq!(stored["locations"]["l1"]["name"], json!("Hauptstr. 1"));
    assert_eq!(stored["description"], json!("Bring the card\nand the form"));
    assert!(s.items().iter().any(|i| i.key == key));
    assert!(sheet_text(&s).contains("Dentist"));
}

#[test]
fn a_new_event_needs_a_title_and_a_time_order_and_the_draft_stays() {
    let w = World::berlin();
    let mut s = w.state();
    s.begin_create();
    s.set_field("location", "somewhere");
    s.save();
    assert!(
        s.editing()
            .unwrap()
            .error
            .as_deref()
            .unwrap()
            .contains("title")
    );
    s.set_field("title", "T");
    s.set_field("end", "2026-11-06 08:00");
    s.save();
    assert!(
        s.editing()
            .unwrap()
            .error
            .as_deref()
            .unwrap()
            .contains("before the start")
    );
    s.set_field("end", "tomorrow");
    s.save();
    assert!(
        s.editing()
            .unwrap()
            .error
            .as_deref()
            .unwrap()
            .contains("not a date")
    );
    s.set_field("end", "2026-11-06 10:00");
    s.set_field("time_zone", "Mars/Olympus");
    s.save();
    assert!(
        s.editing()
            .unwrap()
            .error
            .as_deref()
            .unwrap()
            .contains("not a time zone")
    );
    assert_eq!(
        s.editing().unwrap().draft.value("location"),
        Some("somewhere")
    );
    assert!(
        w.items("2026-11-01", "2026-11-30").is_empty(),
        "nothing stored"
    );
    s.set_field("time_zone", "Asia/Tokyo");
    s.save();
    assert!(s.editing().is_none(), "{:?}", s.notice());
}

#[test]
fn deleting_goes_through_the_core_and_can_be_undone_there() {
    let w = World::berlin();
    let key = w.add_json(rich());
    let mut s = w.state();
    s.select(&key);
    s.confirm_delete();
    assert_eq!(w.items("2026-11-24", "2026-11-24").len(), 1, "not asked");
    s.ask_delete();
    s.ask_delete();
    assert_eq!(w.items("2026-11-24", "2026-11-24").len(), 1, "asking twice");
    s.confirm_delete();
    assert!(w.items("2026-11-24", "2026-11-24").is_empty());
    assert_eq!(s.shown(), &Shown::Nothing);
    assert!(s.notice().unwrap().contains("Kick-off"));
}

#[test]
fn a_change_made_elsewhere_during_an_edit_is_a_conflict_and_only_fields_changed_are_written() {
    let w = World::berlin();
    let key = w.add_json(rich());
    let mut s = w.state();
    s.select(&key);
    s.begin_edit();
    s.set_field("title", "Mine");
    let mut theirs: Event = Event::from_value(w.stored(&key).content).unwrap();
    theirs.description = Some("Theirs".into());
    w.runtime
        .block_on(w.repo.update(key.parse().unwrap(), theirs))
        .unwrap();
    s.save();
    assert!(
        s.editing()
            .unwrap()
            .error
            .as_deref()
            .unwrap()
            .contains("Someone else")
    );
    assert_eq!(w.stored(&key).content["title"], json!("Kick-off"));
    s.save();
    assert!(s.editing().is_none());
    let stored = json_of(&w, &key);
    assert_eq!(stored["title"], json!("Mine"));
    assert_eq!(
        stored["description"],
        json!("Theirs"),
        "their change to another field stays"
    );
}

#[test]
fn after_a_failed_save_and_cancel_the_stored_state_is_shown_and_a_deleted_event_leaves() {
    let w = World::berlin();
    let key = w.add_json(rich());
    let mut s = w.state();
    s.select(&key);
    s.begin_edit();
    s.set_field("end", "not a time");
    s.save();
    assert!(s.editing().unwrap().error.is_some());
    s.cancel_edit();
    s.cancel_edit();
    assert!(sheet_text(&s).contains("Kick-off"));
    assert!(sheet_text(&s).contains("2026-11-24 11:30"));
    s.begin_edit();
    s.set_field("title", "x");
    w.runtime
        .block_on(w.repo.delete(key.parse().unwrap()))
        .unwrap();
    s.save();
    assert!(s.editing().is_none());
    assert_eq!(s.shown(), &Shown::Nothing);
    assert!(s.items().is_empty());
    assert!(s.notice().unwrap().contains("no longer exists"));
}

#[test]
fn the_sheet_names_what_it_has_no_line_for() {
    let w = World::berlin();
    let key = w.add_json(rich());
    let mut s = w.state();
    s.select(&key);
    let text = sheet_text(&s);
    for word in [
        "virtualLocations",
        "alerts",
        "freesynergy.net:source",
        "Main hall",
        "NLnet",
        "First line",
    ] {
        assert!(text.contains(word), "{word} in {text}");
    }
}

#[test]
fn a_key_that_names_nothing_is_not_found_and_capabilities_come_from_the_core() {
    let w = World::berlin();
    assert_eq!(
        w.adapter.read("not a key").unwrap_err().kind,
        fs_components_workbenches::ErrorKind::NotFound
    );
    let can = w.adapter.capabilities();
    assert!(can.edit && can.create && can.delete);
}
