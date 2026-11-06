//! A series keeps itself through an edit: rule, overrides and times, through the real core.
//!
//! Pattern: Round-trip tests — the series as JSON before and after; a change that cannot be done
//! properly is refused and changes nothing.

mod common;

use common::{World, json_of, rich, rich_series};
use fs_calendar_core::Event;
use fs_components_workbenches::calendar::{CalendarItem, CalendarSource, CalendarState, Shown};
use serde_json::json;

fn saved(s: &CalendarState) {
    assert_eq!(
        s.editing().map(|e| e.error.clone()),
        None,
        "{:?}",
        s.notice()
    );
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
            .contains("editing the series")
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
    let text = common::sheet_text(&s);
    assert!(
        text.contains("Series title: Weekly"),
        "the save is to be seen: {text}"
    );
    assert!(text.contains("values of its own"), "{text}");
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
