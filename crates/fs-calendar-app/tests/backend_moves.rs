//! Moving one occurrence (also onto the start of the series), changes without a difference, find against agenda.
//!
//! Pattern: none (tests).
#![allow(clippy::pedantic)]

mod common;

use common::*;
use fs_admin_frontend::Backend;
use fs_calendar_core::Revision;
use fs_objects::ObjectId;

fn starts(w: &World) -> Vec<String> {
    let mut out: Vec<String> = w
        .backend
        .list(&range())
        .unwrap()
        .iter()
        .map(|r| r.values["start"][0].clone())
        .collect();
    out.sort();
    out
}

fn overrides(w: &World, oid: ObjectId) -> usize {
    let e = w.rt.block_on(w.repo.get_event(oid)).unwrap().unwrap();
    e.recurrence_overrides.len()
}

#[test]
fn an_occurrence_can_be_moved_onto_the_start_of_the_series_and_back() {
    let w = world();
    let id = series_id(&w);
    let oid: ObjectId = id.parse().unwrap();
    let key = format!("{id}#2026-12-08T10:00:00");
    w.backend.list(&range()).unwrap();
    w.backend
        .run(
            "change",
            &v(&[("id", &[&key]), ("start", &["2026-11-24T10:00:00"])]),
        )
        .unwrap();
    // the move is stored, not lost: the series has two instances at its own start now
    assert_eq!(
        starts(&w),
        [
            "2026-11-24T10:00:00",
            "2026-11-24T10:00:00",
            "2026-12-01T10:00:00",
            "2026-12-15T10:00:00"
        ]
    );
    assert_eq!(overrides(&w, oid), 1);
    // and back to the day the key names: the override is gone, the series is as it was
    w.backend.list(&range()).unwrap();
    w.backend
        .run(
            "change",
            &v(&[("id", &[&key]), ("start", &["2026-12-08T10:00:00"])]),
        )
        .unwrap();
    assert_eq!(
        starts(&w),
        [
            "2026-11-24T10:00:00",
            "2026-12-01T10:00:00",
            "2026-12-08T10:00:00",
            "2026-12-15T10:00:00"
        ]
    );
    assert_eq!(overrides(&w, oid), 0);
}

#[test]
fn a_change_without_a_difference_writes_nothing_and_keeps_the_revision() {
    let w = world();
    let id = series_id(&w);
    let oid: ObjectId = id.parse().unwrap();
    let key = format!("{id}#2026-12-08T10:00:00");
    w.backend.list(&range()).unwrap();
    let before = w.rt.block_on(w.repo.get(oid)).unwrap().unwrap();
    // the values the form shows, sent back as they are
    let shown = w
        .backend
        .list(&range())
        .unwrap()
        .into_iter()
        .find(|r| r.id == key)
        .unwrap();
    let mut same = shown.values.clone();
    same.insert("id".into(), vec![key.clone()]);
    w.backend.run("change", &same).unwrap();
    let after = w.rt.block_on(w.repo.get(oid)).unwrap().unwrap();
    assert_eq!(after.content, before.content);
    assert_eq!(Revision::of(&after), Revision::of(&before));
    assert_eq!(overrides(&w, oid), 0, "no empty override");
}

#[test]
fn find_and_agenda_give_the_same_instances() {
    let w = world();
    w.backend
        .run(
            "create",
            &v(&[
                ("title", &["One"]),
                ("start", &["2026-12-02T09:00:00"]),
                ("timeZone", &["UTC"]),
            ]),
        )
        .unwrap();
    let r = [
        ("from", &["2026-11-30T00:00:00Z"][..]),
        ("to", &["2026-12-20T00:00:00Z"][..]),
    ];
    let mut found: Vec<String> = w
        .backend
        .list(&v(&r))
        .unwrap()
        .into_iter()
        .map(|r| r.id)
        .collect();
    let mut agenda = w.backend.run("agenda", &v(&r)).unwrap().values["events"].clone();
    found.sort();
    agenda.sort();
    assert_eq!(found.len(), 4);
    assert_eq!(found, agenda);
}
