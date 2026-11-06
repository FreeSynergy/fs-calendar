//! One bad event costs itself only; an override moved far back is found; what cannot be stored is
//! refused without a panic.
//!
//! Pattern: Integration tests over the real core, with what the core lets in.

mod common;

use common::{Scratch, World, d, rich, rich_series, sheet_text};
use fs_calendar_app::import_file;
use fs_calendar_core::Event;
use fs_components_workbenches::ErrorKind;
use fs_components_workbenches::calendar::{CalendarSource, Shown};
use serde_json::json;

/// An endless series with an override the core stores and cannot expand.
fn damaged_series() -> serde_json::Value {
    let mut event = rich_series();
    event["title"] = json!("Bad series");
    event["recurrenceRule"] = json!({"@type": "RecurrenceRule", "frequency": "weekly"});
    event["recurrenceOverrides"] = json!({"2026-11-10T10:00:00": {"duration": "nonsense"}});
    event
}

#[test]
fn one_bad_override_leaves_the_rest_shown_and_is_named_and_deletable() {
    let w = World::berlin();
    let bad = w.add_json(damaged_series());
    w.add_json(rich());
    let core = w.runtime.block_on(w.repo.occurrences(
        fs_calendar_core::utc("2026-11-01T00:00:00Z").unwrap(),
        fs_calendar_core::utc("2026-12-01T00:00:00Z").unwrap(),
    ));
    assert!(
        core.is_err(),
        "the core fails as a whole: that is the case here"
    );
    let mut s = w.state();
    assert_eq!(s.problem(), None, "the calendar is not down");
    assert!(
        s.items().iter().any(|i| i.title == "Kick-off"),
        "the rest is there"
    );
    let named = &s.problems()[0];
    assert!(named.text.contains("Bad series"), "{}", named.text);
    assert_eq!(named.key.as_deref(), Some(bad.as_str()));
    let key = named.key.clone().unwrap();
    s.select(&key);
    assert!(sheet_text(&s).contains("Bad series"), "{:?}", s.shown());
    assert!(s.can_delete());
    s.ask_delete();
    assert!(s.notice().unwrap().contains("all its occurrences go"));
    s.confirm_delete();
    assert!(
        s.problems().is_empty(),
        "gone, and the calendar is whole again"
    );
    assert!(!s.items().is_empty());
}

#[test]
fn an_occurrence_of_a_series_gone_bad_since_says_so_and_warns_before_a_delete() {
    let w = World::berlin();
    let mut event = rich_series();
    event["recurrenceRule"] = json!({"@type": "RecurrenceRule", "frequency": "weekly"});
    let key = w.add_json(event.clone());
    let occurrence = format!("{key}#2026-11-24T10:00:00");
    event["recurrenceOverrides"] = json!({"2026-11-10T10:00:00": {"duration": "nonsense"}});
    w.runtime
        .block_on(
            w.repo
                .update(key.parse().unwrap(), Event::from_value(event).unwrap()),
        )
        .unwrap();
    let mut s = w.state();
    s.select(&occurrence);
    assert!(matches!(s.shown(), Shown::Failed { .. }), "{:?}", s.shown());
    s.ask_delete();
    assert!(
        s.notice().unwrap().contains("all its occurrences go"),
        "{:?}",
        s.notice()
    );
}

#[test]
fn an_override_moved_before_the_series_is_shown_where_it_went_and_opens() {
    let w = World::berlin();
    let mut event = rich_series();
    event["recurrenceOverrides"] = json!({
        "2026-11-10T10:00:00": {"title": "Moved far back", "start": "2026-09-01T10:00:00"}
    });
    let key = w.add_json(event);
    let moved = w.item("2026-09-01", "Moved far back");
    assert_eq!(moved.key, format!("{key}#2026-11-10T10:00:00"));
    assert_eq!(w.items("2026-09-01", "2026-09-07").len(), 1, "in its week");
    assert!(
        w.items("2026-11-10", "2026-11-10").is_empty(),
        "not where it was"
    );
    assert_eq!(
        w.items("2026-11-03", "2026-11-03").len(),
        1,
        "and only once where the rest of the series is"
    );
    let mut s = w.state();
    s.select(&moved.key);
    assert!(sheet_text(&s).contains("Moved far back"), "{:?}", s.shown());
    let mut wide = w.state();
    wide.go_to(d("2026-09-15"));
    wide.select(&moved.key);
    assert!(
        matches!(wide.shown(), Shown::Item { .. }),
        "{:?}",
        wide.shown()
    );
}

#[test]
fn an_override_moved_back_after_the_program_started_is_seen_after_own_writes() {
    let w = World::berlin();
    let key = w.add_json(rich_series());
    assert!(w.items("2026-09-01", "2026-09-07").is_empty());
    let mut event = w.stored(&key).content;
    event["recurrenceOverrides"] = json!({
        "2026-11-10T10:00:00": {"title": "Back", "start": "2026-09-01T10:00:00"}
    });
    // Through the adapter's own save the remembered list is forgotten; a title edit stands for it.
    w.runtime
        .block_on(
            w.repo
                .update(key.parse().unwrap(), Event::from_value(event).unwrap()),
        )
        .unwrap();
    let mut draft = w.adapter.draft(&key).unwrap();
    let title = draft.fields.iter_mut().find(|f| f.key == "title").unwrap();
    title.value = "Renamed".into();
    w.adapter.save(&key, &draft).unwrap();
    assert_eq!(w.items("2026-09-01", "2026-09-07").len(), 1);
}

#[test]
fn a_length_far_too_long_stops_an_import_with_words_and_no_panic_reaches_the_caller() {
    let (w, dir) = (World::berlin(), Scratch::new());
    let file = dir.file("long.ics");
    std::fs::write(
        &file,
        "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//x//y//EN\r\nBEGIN:VEVENT\r\nUID:long-1\r\nDTSTAMP:20261101T000000Z\r\nDTSTART:20261130T090000Z\r\nDURATION:P99999999999D\r\nSUMMARY:Long\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n",
    )
    .unwrap();
    let summary = import_file(w.repo.as_ref(), w.runtime.handle(), &file).unwrap();
    eprintln!("{}", summary.message());
    assert!(!summary.is_clean(), "{}", summary.message());
}

#[test]
fn a_bad_key_of_a_task_or_event_is_not_found() {
    let w = World::berlin();
    for key in ["task:nope", "task:x@2026-11-06", "not-an-id#x"] {
        assert_eq!(
            w.adapter.read(key).unwrap_err().kind,
            ErrorKind::NotFound,
            "{key}"
        );
    }
}
