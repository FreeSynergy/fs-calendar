//! `EventBackend` over a real in-memory calendar: commands one by one, the partial change rule, refusals.
//!
//! Pattern: none (tests).
#![allow(clippy::pedantic)]

use std::sync::Arc;

use fs_admin_frontend::{Backend, BackendError, Values};
use fs_calendar_app::EventBackend;
use fs_calendar_core::{Calendar, CalendarRepo};
use fs_objects::{Store, TursoStore};
use fs_types_frontend::RecordProblem;

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

fn create(w: &World, title: &str) -> String {
    let input = v(&[
        ("title", &[title]),
        ("start", &["2026-11-12T09:00:00"]),
        ("timeZone", &["Europe/Berlin"]),
        ("duration", &["PT1H"]),
        ("description", &["Agenda"]),
        ("keywords", &["work, team"]),
    ]);
    w.backend.run("create", &input).unwrap().id.unwrap()
}

#[test]
fn create_then_list() {
    let w = world();
    let id = create(&w, "Standup");
    assert!(id.starts_with("urn:uuid:"));
    let list = w.backend.list(&Values::new()).unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, id);
    assert_eq!(list[0].values["title"], ["Standup"]);
    assert_eq!(
        list[0].values["keywords"],
        ["team, work".to_owned()].map(|_| "team, work".to_owned())
    );
}

#[test]
fn change_is_partial() {
    let w = world();
    let id = create(&w, "Standup");
    w.backend.list(&Values::new()).unwrap();
    // only the title is sent: everything else stays
    w.backend
        .run("change", &v(&[("id", &[&id]), ("title", &["Daily"])]))
        .unwrap();
    let r = &w.backend.list(&Values::new()).unwrap()[0];
    assert_eq!(r.values["title"], ["Daily"]);
    assert_eq!(r.values["description"], ["Agenda"]);
    assert_eq!(r.values["duration"], ["PT1H"]);
    // an empty list clears the field
    w.backend
        .run("change", &v(&[("id", &[&id]), ("description", &[])]))
        .unwrap();
    let r = &w.backend.list(&Values::new()).unwrap()[0];
    assert!(!r.values.contains_key("description"));
    assert_eq!(r.values["title"], ["Daily"]);
    // values replace
    w.backend
        .run(
            "change",
            &v(&[
                ("id", &[&id]),
                ("keywords", &["solo"]),
                ("locations", &["geo:48.2,16.37"]),
                ("links", &["https://example.org/x"]),
            ]),
        )
        .unwrap();
    let r = &w.backend.list(&Values::new()).unwrap()[0];
    assert_eq!(r.values["keywords"], ["solo"]);
    assert_eq!(r.values["locations"], ["geo:48.2,16.37"]);
    assert_eq!(r.values["links"], ["https://example.org/x"]);
}

#[test]
fn a_named_place_survives_an_edit_of_the_places() {
    let w = world();
    let id = create(&w, "Offsite");
    let oid: fs_objects::ObjectId = id.parse().unwrap();
    let mut e = w.rt.block_on(w.calendar.get_event(oid)).unwrap().unwrap();
    e.extra.insert(
        "locations".into(),
        serde_json::json!({"a": {"@type": "Location", "name": "Hall B"}}),
    );
    w.rt.block_on(w.calendar.update(oid, e)).unwrap();
    w.backend.list(&Values::new()).unwrap(); // read again
    w.backend
        .run("change", &v(&[("id", &[&id]), ("locations", &["geo:1,2"])]))
        .unwrap();
    let e = w.rt.block_on(w.calendar.get_event(oid)).unwrap().unwrap();
    let places = e.extra["locations"].as_object().unwrap();
    assert_eq!(places.len(), 2);
    assert!(places.values().any(|p| p["name"] == "Hall B"));
}

#[test]
fn a_change_after_somebody_else_changed_it_is_refused() {
    let w = world();
    let id = create(&w, "Standup");
    w.backend.list(&Values::new()).unwrap(); // the person saw this revision
    let oid: fs_objects::ObjectId = id.parse().unwrap();
    let mut e = w.rt.block_on(w.calendar.get_event(oid)).unwrap().unwrap();
    e.title = Some("Other".into());
    w.rt.block_on(w.calendar.update(oid, e)).unwrap();
    match w
        .backend
        .run("change", &v(&[("id", &[&id]), ("title", &["Mine"])]))
    {
        Err(BackendError::Unavailable(m)) => assert!(m.starts_with("conflict"), "{m}"),
        other => panic!("{other:?}"),
    }
    let r = &w.backend.list(&Values::new()).unwrap()[0];
    assert_eq!(r.values["title"], ["Other"]);
}

#[test]
fn a_bad_event_is_rejected_at_the_field() {
    let w = world();
    let bad = v(&[("title", &["x"]), ("start", &["next tuesday"])]);
    match w.backend.run("create", &bad) {
        Err(BackendError::Rejected(issues)) => {
            assert!(issues.iter().any(|i| i.field == "start"), "{issues:?}")
        }
        other => panic!("{other:?}"),
    }
    let no_start = v(&[("title", &["x"])]);
    match w.backend.run("create", &no_start) {
        Err(BackendError::Rejected(issues)) => assert!(
            issues
                .iter()
                .any(|i| i.field == "start" && i.problem == RecordProblem::Missing),
            "{issues:?}"
        ),
        other => panic!("{other:?}"),
    }
    let id = create(&w, "Ok");
    let r = w
        .backend
        .run("change", &v(&[("id", &[&id]), ("duration", &["soon"])]));
    assert!(matches!(r, Err(BackendError::Rejected(i)) if i[0].field == "duration"));
    let r = w
        .backend
        .run("change", &v(&[("id", &[&id]), ("priority", &["high"])]));
    assert!(matches!(r, Err(BackendError::Rejected(i)) if i[0].field == "priority"));
}
