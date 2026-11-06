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

fn moved_elsewhere(w: &World, key: &str, change: impl Fn(&mut Event)) {
    let mut theirs = Event::from_value(w.stored(key).content).unwrap();
    change(&mut theirs);
    w.runtime
        .block_on(w.repo.update(key.parse().unwrap(), theirs))
        .unwrap();
}

/// The form with `typed`, the event changed elsewhere, then Save twice.
fn clash(typed: &[(&str, &str)], change: impl Fn(&mut Event)) -> String {
    let w = World::berlin();
    let key = w.add_json(rich());
    let mut s = w.state();
    s.select(&key);
    s.begin_edit();
    for (field, value) in typed {
        s.set_field(field, value);
    }
    moved_elsewhere(&w, &key, change);
    let before = w.stored(&key).content;
    s.save();
    let first = s
        .editing()
        .expect("refused, draft kept")
        .error
        .clone()
        .unwrap();
    assert!(first.contains("changed elsewhere"), "{first}");
    assert!(
        !first.contains("Press Save again"),
        "no offer to write it anyway: {first}"
    );
    s.save();
    let edit = s.editing().expect("still refused");
    for (field, value) in typed {
        assert_eq!(edit.draft.value(field), Some(*value), "the draft stays");
    }
    assert_eq!(w.stored(&key).content, before, "nothing was written");
    first
}

#[test]
fn a_start_moved_against_an_event_moved_is_refused_and_says_what_moved() {
    let said = clash(&[("start", "2026-11-24 09:00")], |theirs| {
        theirs.start = Some("2026-12-24T10:00:00".into());
    });
    assert!(said.contains("Start is now 2026-12-24 10:00"), "{said}");
    assert!(said.contains("End is now 2026-12-24 11:30"), "{said}");
}

#[test]
fn a_typed_end_against_all_day_is_refused() {
    let said = clash(&[("end", "2026-11-24 12:00")], |theirs| {
        theirs.show_without_time = Some(true);
        theirs.start = Some("2026-11-24T00:00:00".into());
        theirs.time_zone = None;
        theirs.duration = Some("P1D".into());
    });
    assert!(said.contains("All day is now Yes"), "{said}");
}

#[test]
fn a_changed_start_against_a_changed_zone_is_refused() {
    let said = clash(&[("start", "2026-11-24 09:00")], |theirs| {
        theirs.time_zone = Some("Asia/Tokyo".into());
    });
    assert!(said.contains("Time zone is now Asia/Tokyo"), "{said}");
}

#[test]
fn a_changed_zone_against_a_length_changed_is_refused() {
    let said = clash(&[("time_zone", "Europe/Paris")], |theirs| {
        theirs.duration = Some("PT3H".into());
    });
    assert!(said.contains("End is now 2026-11-24 13:00"), "{said}");
}

#[test]
fn only_a_title_typed_against_an_event_moved_merges_and_times_typed_against_a_title_too() {
    let w = World::berlin();
    let key = w.add_json(rich());
    let mut s = w.state();
    s.select(&key);
    s.begin_edit();
    s.set_field("title", "Mine");
    moved_elsewhere(&w, &key, |theirs| {
        theirs.start = Some("2026-12-01T08:00:00".into())
    });
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
    assert_eq!(stored["start"], json!("2026-12-01T08:00:00"));
    assert_eq!(stored["title"], json!("Mine"));

    s.begin_edit();
    s.set_field("end", "2026-12-01 09:00");
    moved_elsewhere(&w, &key, |theirs| theirs.title = Some("Theirs".into()));
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
    assert_eq!(stored["duration"], json!("PT1H"));
    assert_eq!(stored["title"], json!("Theirs"));
}

#[test]
fn putting_the_times_back_saves_the_rest_on_the_newest() {
    let w = World::berlin();
    let key = w.add_json(rich());
    let mut s = w.state();
    s.select(&key);
    s.begin_edit();
    s.set_field("title", "Mine");
    s.set_field("start", "2026-11-24 09:00");
    moved_elsewhere(&w, &key, |theirs| {
        theirs.start = Some("2026-12-24T10:00:00".into())
    });
    s.save();
    assert!(s.editing().unwrap().error.is_some());
    s.set_field("start", "2026-11-24 10:00");
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
    assert_eq!(stored["start"], json!("2026-12-24T10:00:00"));
    assert_eq!(stored["title"], json!("Mine"));
}
