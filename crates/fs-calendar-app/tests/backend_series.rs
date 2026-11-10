//! Series: one occurrence is changed or deleted as an override, the series stays; conflicts and refusals.
//!
//! Pattern: none (tests).
#![allow(clippy::pedantic)]

use std::sync::Arc;

use fs_admin_frontend::{Backend, BackendError, Values};
use fs_calendar_app::EventBackend;
use fs_calendar_core::{Calendar, CalendarRepo};
use fs_objects::{Store, TursoStore};

const ICS: &str = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//x//y//EN\r\nBEGIN:VEVENT\r\nUID:ev-1\r\nDTSTAMP:20261101T000000Z\r\nDTSTART;TZID=Europe/Berlin:20261124T100000\r\nDTEND;TZID=Europe/Berlin:20261124T113000\r\nSUMMARY:Jour fixe\r\nRRULE:FREQ=WEEKLY;COUNT=4;BYDAY=TU\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";

struct World {
    rt: tokio::runtime::Runtime,
    calendar: Arc<CalendarRepo>,
    backend: EventBackend,
}

fn world() -> World {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let store: Arc<dyn Store> = Arc::new(rt.block_on(TursoStore::open_in_memory()).unwrap());
    let calendar = Arc::new(CalendarRepo::new(
        store,
        "https://local.invalid/users/owner",
    ));
    rt.block_on(calendar.import_ics(ICS)).unwrap();
    let backend = EventBackend::new(calendar.clone() as Arc<dyn Calendar>, rt.handle().clone());
    World {
        rt,
        calendar,
        backend,
    }
}

fn v(pairs: &[(&str, &[&str])]) -> Values {
    pairs
        .iter()
        .map(|(k, vs)| {
            (
                (*k).to_owned(),
                vs.iter().map(|s| (*s).to_owned()).collect(),
            )
        })
        .collect()
}

fn range() -> Values {
    v(&[
        ("from", &["2026-11-01T00:00:00Z"]),
        ("to", &["2027-01-31T00:00:00Z"]),
    ])
}

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

fn instance(w: &World, start: &str) -> String {
    w.backend
        .list(&range())
        .unwrap()
        .into_iter()
        .find(|r| r.values["start"][0] == start)
        .unwrap()
        .id
}

#[test]
fn changing_one_occurrence_leaves_the_series() {
    let w = world();
    let id = instance(&w, "2026-12-08T10:00:00");
    assert!(id.contains('#'));
    w.backend
        .run(
            "change",
            &v(&[
                ("id", &[&id]),
                ("start", &["2026-12-08T11:00:00"]),
                ("title", &["Moved"]),
            ]),
        )
        .unwrap();
    assert_eq!(
        starts(&w),
        [
            "2026-11-24T10:00:00",
            "2026-12-01T10:00:00",
            "2026-12-08T11:00:00",
            "2026-12-15T10:00:00"
        ]
    );
    let moved = w
        .backend
        .list(&range())
        .unwrap()
        .into_iter()
        .find(|r| r.values["start"][0] == "2026-12-08T11:00:00")
        .unwrap();
    assert_eq!(moved.values["title"], ["Moved"]);
    // the master is untouched
    let all = w.backend.list(&Values::new()).unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].values["start"], ["2026-11-24T10:00:00"]);
    assert_eq!(all[0].values["title"], ["Jour fixe"]);
    // a second edit of the same instance keeps the first
    let id = instance(&w, "2026-12-08T11:00:00");
    w.backend
        .run(
            "change",
            &v(&[("id", &[&id]), ("description", &["Room 4"])]),
        )
        .unwrap();
    let again = w
        .backend
        .list(&range())
        .unwrap()
        .into_iter()
        .find(|r| r.values["start"][0] == "2026-12-08T11:00:00")
        .unwrap();
    assert_eq!(again.values["title"], ["Moved"]);
    assert_eq!(again.values["description"], ["Room 4"]);
}

#[test]
fn deleting_one_occurrence_leaves_the_series() {
    let w = world();
    let id = instance(&w, "2026-12-08T10:00:00");
    w.backend.run("delete", &v(&[("id", &[&id])])).unwrap();
    assert_eq!(
        starts(&w),
        [
            "2026-11-24T10:00:00",
            "2026-12-01T10:00:00",
            "2026-12-15T10:00:00"
        ]
    );
    assert_eq!(w.backend.list(&Values::new()).unwrap().len(), 1);
}

#[test]
fn a_change_after_a_range_read_conflicts_with_somebody_elses_change() {
    let w = world();
    let id = instance(&w, "2026-12-01T10:00:00"); // read by range: the revision is remembered
    let master: fs_objects::ObjectId = id.split('#').next().unwrap().parse().unwrap();
    let mut e =
        w.rt.block_on(w.calendar.get_event(master))
            .unwrap()
            .unwrap();
    e.title = Some("Other".into());
    w.rt.block_on(w.calendar.update(master, e)).unwrap();
    match w
        .backend
        .run("change", &v(&[("id", &[&id]), ("title", &["Mine"])]))
    {
        Err(BackendError::Unavailable(m)) => assert!(m.starts_with("conflict"), "{m}"),
        other => panic!("{other:?}"),
    }
    let e =
        w.rt.block_on(w.calendar.get_event(master))
            .unwrap()
            .unwrap();
    assert_eq!(e.title.as_deref(), Some("Other"));
    assert!(e.recurrence_overrides.is_empty());
}

#[test]
fn two_participants_with_one_address_stay_two() {
    let w = world();
    let id = w.backend.list(&Values::new()).unwrap()[0].id.clone();
    let oid: fs_objects::ObjectId = id.parse().unwrap();
    let mut e = w.rt.block_on(w.calendar.get_event(oid)).unwrap().unwrap();
    e.add_invitee("Ben", "ben@example.org");
    e.add_invitee("Ben again", "ben@example.org");
    w.rt.block_on(w.calendar.update(oid, e)).unwrap();
    w.backend.list(&Values::new()).unwrap();
    w.backend
        .run(
            "change",
            &v(&[
                ("id", &[&id]),
                (
                    "participants",
                    &["mailto:ben@example.org", "mailto:ann@example.org"],
                ),
            ]),
        )
        .unwrap();
    let e = w.rt.block_on(w.calendar.get_event(oid)).unwrap().unwrap();
    assert_eq!(e.participants.len(), 3);
}
