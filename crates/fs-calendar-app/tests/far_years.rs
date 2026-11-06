//! Years beyond the calendar are refused with words, and its far edge works.
//!
//! Pattern: Table test over the real core.

mod common;

use common::World;
use fs_calendar_core::Event;
use fs_components_workbenches::calendar::CalendarState;

fn timed(title: &str, start: &str, zone: &str, duration: &str) -> Event {
    Event::new(title, start, zone, duration)
}

fn edit(w: &World, key: &str, change: impl Fn(&mut CalendarState)) -> CalendarState {
    let mut s = w.state();
    s.select(key);
    s.begin_edit();
    change(&mut s);
    s.save();
    s
}

fn error_of(s: &CalendarState) -> String {
    s.editing()
        .and_then(|e| e.error.clone())
        .unwrap_or_default()
}

#[test]
fn years_beyond_the_calendar_are_refused_with_words_and_the_far_edge_works() {
    let w = World::berlin();
    let key = w.add(timed("Far", "2026-11-24T10:00:00", "Europe/Berlin", "PT1H"));
    let before = w.stored(&key).content;
    for typed in [
        "9001-01-01 10:00",
        "0000-06-01 10:00",
        "-500-01-01 10:00",
        "99999-01-01 10:00",
        "40000-01-01",
    ] {
        let s = edit(&w, &key, |s| s.set_field("end", typed));
        assert!(!error_of(&s).is_empty(), "{typed}");
        assert_eq!(w.stored(&key).content, before, "{typed}");
    }
    let s = edit(&w, &key, |s| s.set_field("end", "9000-12-31 23:00"));
    assert!(s.editing().is_none(), "the far edge: {:?}", s.notice());
    let mut s = w.state();
    s.begin_create();
    s.set_field("title", "From the start");
    s.set_field("start", "0001-01-01 10:00");
    s.set_field("end", "9000-12-31 10:00");
    s.toggle_field("all_day");
    s.save();
    assert!(
        s.editing().is_none(),
        "the whole range all day: {:?}",
        s.notice()
    );
}
