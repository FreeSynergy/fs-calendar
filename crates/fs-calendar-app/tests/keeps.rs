//! Editing keeps everything else on the event, through the real core and the real state machine.
//!
//! Pattern: Round-trip tests — the event as JSON before and after, compared but for what was edited.

mod common;

use common::{World, d, json_of, rich, rich_series};
use fs_calendar_core::Event;
use fs_components_workbenches::calendar::{CalendarItem, CalendarSource, CalendarState, Shown};
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

fn occurrence(w: &World, day: &str, title: &str) -> CalendarItem {
    w.item(day, title)
}

#[test]
fn a_series_shows_its_occurrences_with_their_override_and_without_the_excluded() {
    let w = World::berlin();
    let key = w.add_json(rich_series());
    let on = |day: &str| w.items(day, day);
    assert_eq!(on("2026-11-03")[0].title, "Jour fixe");
    assert!(on("2026-11-10").is_empty(), "moved away");
    let moved = occurrence(&w, "2026-11-11", "Moved jour fixe");
    assert_eq!(moved.start.clock(), "15:00");
    assert!(on("2026-11-17").is_empty(), "excluded");
    assert_eq!(
        on("2026-11-24")[0].key,
        format!("{key}#2026-11-24T10:00:00")
    );
    assert!(on("2027-01-05").is_empty(), "after the last of eight");
    assert_eq!(on("2026-12-22").len(), 1, "the eighth");
}

#[test]
fn editing_from_an_occurrence_edits_the_series_and_never_writes_the_occurrence_over_it() {
    let w = World::berlin();
    let key = w.add_json(rich_series());
    let before = json_of(&w, &key);
    let moved = occurrence(&w, "2026-11-11", "Moved jour fixe");
    let mut s = w.state();
    s.select(&moved.key);
    let Shown::Item { sheet, .. } = s.shown() else {
        panic!("{:?}", s.shown())
    };
    assert_eq!(
        sheet.title, "Moved jour fixe",
        "the sheet shows the occurrence"
    );
    assert!(sheet.note.as_deref().unwrap().contains("repeats"));
    s.begin_edit();
    let edit = s.editing().unwrap();
    assert_eq!(
        edit.draft.value("title"),
        Some("Jour fixe"),
        "the form shows the series"
    );
    assert!(
        edit.draft
            .note
            .as_deref()
            .unwrap()
            .contains("all occurrences")
    );
    s.set_field("title", "Weekly");
    s.save();
    saved(&s);
    let mut expected = before;
    expected["title"] = json!("Weekly");
    assert_eq!(
        json_of(&w, &key),
        expected,
        "rule, overrides, start all as they were"
    );
    assert_eq!(
        occurrence(&w, "2026-11-11", "Moved jour fixe")
            .start
            .clock(),
        "15:00"
    );
    assert_eq!(
        occurrence(&w, "2026-11-03", "Weekly").start.clock(),
        "10:00"
    );
    assert_eq!(sheet_title(&s), "Moved jour fixe", "still that occurrence");
}

fn sheet_title(s: &CalendarState) -> String {
    match s.shown() {
        Shown::Item { sheet, .. } => sheet.title.clone(),
        other => format!("{other:?}"),
    }
}

#[test]
fn the_times_of_a_series_are_locked_and_a_save_that_changes_them_is_refused() {
    let w = World::berlin();
    let key = w.add_json(rich_series());
    let before = json_of(&w, &key);
    let item = occurrence(&w, "2026-11-03", "Jour fixe");
    let mut s = w.state();
    s.select(&item.key);
    s.begin_edit();
    for field in ["start", "end", "all_day", "time_zone"] {
        s.set_field(field, "2026-11-04 10:00");
        s.toggle_field(field);
    }
    assert!(!s.editing().unwrap().draft.changed(), "locked in the form");
    // A form that gets past that, as another frontend might: the source refuses it clearly.
    let mut draft = w.adapter.draft(&item.key).unwrap();
    let start = draft.fields.iter_mut().find(|f| f.key == "start").unwrap();
    start.value = "2026-11-04 11:00".into();
    let refused = w.adapter.save(&item.key, &draft).unwrap_err();
    assert!(refused.message.contains("repeats"), "{refused}");
    assert_eq!(json_of(&w, &key), before, "nothing was changed");
    let mut event: Event = Event::from_value(w.stored(&key).content).unwrap();
    let held = event.clone();
    let also = fs_calendar_app::apply_draft(&mut event, &draft);
    assert!(also.is_err());
    assert_eq!(event, held, "a refusal leaves the event as it was");
}

#[test]
fn one_occurrence_gone_says_so_and_deleting_says_it_takes_the_series() {
    let w = World::berlin();
    let key = w.add_json(rich_series());
    let item = occurrence(&w, "2026-11-24", "Jour fixe");
    let mut s = w.state();
    s.select(&item.key);
    s.ask_delete();
    assert!(s.notice().unwrap().contains("all its occurrences go"));
    s.keep();
    let mut held = w.stored(&key).content;
    held["recurrenceOverrides"]["2026-11-24T10:00:00"] = json!({"excluded": true});
    w.runtime
        .block_on(
            w.repo
                .update(key.parse().unwrap(), Event::from_value(held).unwrap()),
        )
        .unwrap();
    s.reload();
    assert!(s.items().iter().all(|i| i.key != item.key));
    let mut fresh = w.state();
    fresh.select(&item.key);
    assert_eq!(fresh.shown(), &Shown::Nothing);
    assert!(fresh.notice().unwrap().contains("no longer exists"));
}

#[test]
fn a_form_field_nobody_knows_is_refused_not_dropped() {
    let w = World::berlin();
    let key = w.add_json(rich());
    let mut draft = w.adapter.draft(&key).unwrap();
    draft
        .fields
        .push(fs_components_workbenches::calendar::DraftField::new(
            "nope", "Nope", "",
        ));
    draft.fields.last_mut().unwrap().value = "x".into();
    assert!(
        w.adapter
            .save(&key, &draft)
            .unwrap_err()
            .message
            .contains("unknown field")
    );
}
