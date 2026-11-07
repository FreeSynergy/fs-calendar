//! A sensitive marking is seen wherever the event appears and no save reduces it (ADR 0029).
//!
//! Round-trip tests through the real core and the real state machine.

mod common;

use common::{World, rich};
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

/// An event marked in the content, and one on the envelope only.
fn marked_json() -> serde_json::Value {
    let mut event = rich();
    event["freesynergy.net:sensitive"] = json!(["pd:HealthRecord"]);
    event
}

#[test]
fn a_marking_in_the_content_stays_and_is_seen_in_the_view_the_sheet_and_the_form() {
    let w = World::berlin();
    let key = w.add_json(marked_json());
    let item = w.item("2026-11-24", "Kick-off");
    assert_eq!(item.marks, ["Sensitive"]);
    let mut s = w.state();
    assert_eq!(s.shown(), &Shown::Nothing, "never opened on its own");
    s.select(&key);
    let Shown::Item { sheet, .. } = s.shown() else {
        panic!()
    };
    assert_eq!(sheet.marks, ["Sensitive"]);
    s.begin_edit();
    assert_eq!(s.editing().unwrap().draft.marks, ["Sensitive"]);
    s.set_field("title", "Other");
    s.set_field("location", "");
    s.set_field("description", "");
    s.save();
    saved(&s);
    let stored = w.stored(&key).content;
    assert_eq!(
        stored["freesynergy.net:sensitive"],
        json!(["pd:HealthRecord"])
    );
}

#[test]
fn a_marking_on_a_location_survives_emptying_it() {
    let w = World::berlin();
    let mut event = rich();
    event["locations"] = json!({"l1": {"@type": "Location", "name": "Clinic", "freesynergy.net:sensitive": ["pd:HealthRecord"]}});
    let key = w.add_json(event);
    assert_eq!(w.item("2026-11-24", "Kick-off").marks, ["Sensitive"]);
    let mut s = open_event(&w, &key);
    s.begin_edit();
    s.set_field("location", "");
    s.save();
    saved(&s);
    assert!(
        w.stored(&key)
            .content
            .to_string()
            .contains("freesynergy.net:sensitive")
    );
    assert_eq!(w.item("2026-11-24", "Kick-off").marks, ["Sensitive"]);
}

#[test]
fn a_marking_on_the_envelope_stays_through_an_edit() {
    let w = World::berlin();
    let key = w.add_json(rich());
    let mut object = w.stored(&key);
    object.envelope.sensitive = vec!["pd:HealthRecord".into()];
    w.runtime.block_on(w.store.put(object, common::ME)).unwrap();
    assert_eq!(w.item("2026-11-24", "Kick-off").marks, ["Sensitive"]);
    let mut s = open_event(&w, &key);
    s.begin_edit();
    s.set_field("title", "Other");
    s.save();
    saved(&s);
    assert_eq!(w.stored(&key).envelope.sensitive, ["pd:HealthRecord"]);
    assert_eq!(w.item("2026-11-24", "Other").marks, ["Sensitive"]);
}

#[test]
fn private_and_secret_count_as_marked_public_does_not() {
    let w = World::berlin();
    for (i, privacy) in ["private", "secret", "public"].into_iter().enumerate() {
        let mut event = rich();
        event["uid"] = json!(format!("urn:uuid:01a1aaf0-08a0-756c-85b2-f092789b0b0{i}"));
        event["title"] = json!(privacy);
        event["privacy"] = json!(privacy);
        w.add_json(event);
    }
    assert_eq!(w.item("2026-11-24", "private").marks, ["Sensitive"]);
    assert_eq!(w.item("2026-11-24", "secret").marks, ["Sensitive"]);
    assert!(w.item("2026-11-24", "public").marks.is_empty());
}
