//! A change made elsewhere during an edit: a conflict, and then only what the person changed.
//!
//! Pattern: State machine tests over the real core.

mod common;

use common::{World, json_of, rich};
use fs_calendar_core::Event;
use serde_json::json;

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
fn a_forced_save_writes_what_the_person_changed_on_top_of_the_newest() {
    let w = World::berlin();
    let key = w.add_json(rich());
    let mut s = w.state();
    s.select(&key);
    s.begin_edit();
    s.set_field("end", "2026-11-24 12:00");
    let mut theirs = Event::from_value(w.stored(&key).content).unwrap();
    theirs.start = Some("2026-11-24T11:00:00".into());
    theirs.duration = Some("PT2H".into());
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
    s.save();
    assert!(s.editing().is_none(), "{:?}", s.notice());
    let stored = json_of(&w, &key);
    assert_eq!(
        stored["start"],
        json!("2026-11-24T11:00:00"),
        "their start, not the form's old one"
    );
    assert_eq!(
        stored["duration"],
        json!("PT1H"),
        "to the end the person typed"
    );
}

#[test]
fn a_forced_save_of_the_title_does_not_move_an_event_moved_elsewhere() {
    let w = World::berlin();
    let key = w.add_json(rich());
    let mut s = w.state();
    s.select(&key);
    s.begin_edit();
    s.set_field("title", "Mine");
    s.set_field("time_zone", "Europe/Paris");
    let mut theirs = Event::from_value(w.stored(&key).content).unwrap();
    theirs.start = Some("2026-12-01T08:00:00".into());
    w.runtime
        .block_on(w.repo.update(key.parse().unwrap(), theirs))
        .unwrap();
    s.save();
    s.save();
    assert!(s.editing().is_none(), "{:?}", s.notice());
    let stored = json_of(&w, &key);
    assert_eq!(stored["start"], json!("2026-12-01T08:00:00"));
    assert_eq!(stored["duration"], json!("PT1H30M"));
    assert_eq!(stored["title"], json!("Mine"));
}
