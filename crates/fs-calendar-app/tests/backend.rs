//! `EventBackend` over a real in-memory calendar: commands one by one, the partial change rule, refusals.
//!
//! Pattern: none (tests).
#![allow(clippy::pedantic)]

use std::sync::Arc;

use fs_admin_frontend::{Backend, BackendError, Values};
use fs_calendar_app::EventBackend;
use fs_calendar_core::{Calendar, CalendarRepo, support};
use fs_objects::{Store, TursoStore};
use fs_types::RecordProblem;

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

#[test]
fn delete_and_missing_records() {
    let w = world();
    let id = create(&w, "Gone");
    w.backend.run("delete", &v(&[("id", &[&id])])).unwrap();
    assert!(w.backend.list(&Values::new()).unwrap().is_empty());
    assert_eq!(
        w.backend
            .run("change", &v(&[("id", &[&id]), ("title", &["x"])])),
        Err(BackendError::NotFound)
    );
    assert!(matches!(
        w.backend.run("delete", &v(&[("id", &["nonsense"])])),
        Err(BackendError::Rejected(_))
    ));
    assert!(matches!(
        w.backend.run("delete", &Values::new()),
        Err(BackendError::Rejected(_))
    ));
}

#[test]
fn find_filters_by_term_and_by_range() {
    let w = world();
    create(&w, "Standup");
    create(&w, "Lunch");
    let q = v(&[("term", &["STAND"])]);
    assert_eq!(w.backend.list(&q).unwrap().len(), 1);
    let found = w.backend.run("find", &q).unwrap();
    assert_eq!(found.values["events"].len(), 1);
    let report = w.rt.block_on(w.calendar.import_ics(ICS)).unwrap();
    assert_eq!(report.created.len(), 1);
    let range = v(&[
        ("from", &["2026-11-23T00:00:00Z"]),
        ("to", &["2026-12-09T00:00:00Z"]),
    ]);
    let found = w.backend.list(&range).unwrap();
    let series: Vec<_> = found
        .iter()
        .filter(|r| r.values["title"] == ["Jour fixe"])
        .collect();
    assert_eq!(series.len(), 3, "{found:?}");
    assert!(series.iter().all(|r| r.id.contains('#')));
    // an occurrence names its series: change works on it
    let id = series[0].id.clone();
    w.backend
        .run("change", &v(&[("id", &[&id]), ("title", &["Jour fixe!"])]))
        .unwrap();
    // one end alone is a refusal naming the other
    let r = w.backend.list(&v(&[("from", &["2026-11-23T00:00:00Z"])]));
    assert!(matches!(r, Err(BackendError::Rejected(i)) if i[0].field == "to"));
}

#[test]
fn every_unsupported_command_is_refused_and_unknown_ones_are_not_found() {
    let w = world();
    let mut n = 0;
    for r in support::TABLE.iter().filter(|r| !r.supported) {
        n += 1;
        assert_eq!(
            w.backend.run(r.command, &Values::new()),
            Err(BackendError::Unavailable(format!(
                "unsupported: {}",
                r.command
            )))
        );
    }
    assert_eq!(n, 7);
    assert_eq!(
        w.backend.run("nothing", &Values::new()),
        Err(BackendError::NotFound)
    );
}

#[test]
fn invitations_check_and_transfer() {
    let w = world();
    let id = create(&w, "Meeting");
    w.backend
        .run(
            "invite",
            &v(&[
                ("id", &[&id]),
                ("name", &["Ben"]),
                ("email", &["ben@example.org"]),
            ]),
        )
        .unwrap();
    let r = &w.backend.list(&Values::new()).unwrap()[0];
    assert_eq!(r.values["participants"], ["mailto:ben@example.org"]);
    w.backend
        .run(
            "respond",
            &v(&[
                ("id", &[&id]),
                ("who", &["ben@example.org"]),
                ("response", &["accept"]),
                ("comment", &["yes"]),
            ]),
        )
        .unwrap();
    let h = w.backend.run("history", &v(&[("id", &[&id])])).unwrap();
    let entries = &h.values["entries"];
    assert_eq!(entries.len(), 2, "{entries:?}");
    assert!(entries[0].contains("Invite") && entries[1].contains("Accept"));
    let c = w.backend.run("check", &v(&[("id", &[&id])])).unwrap();
    assert!(c.values["problems"].is_empty());
    let ex = w
        .backend
        .run("export", &v(&[("ids", &[&id]), ("format", &["ics"])]))
        .unwrap();
    assert!(ex.values["text"][0].contains("SUMMARY:Meeting"));
    let im = w
        .backend
        .run("import", &v(&[("format", &["ics"]), ("text", &[ICS])]))
        .unwrap();
    assert_eq!(im.values["created"].len(), 1);
    let again = w
        .backend
        .run("import", &v(&[("format", &["ics"]), ("text", &[ICS])]))
        .unwrap();
    assert!(again.values["created"].is_empty());
    assert_eq!(again.values["skipped"].len(), 1);
    let ag = w
        .backend
        .run(
            "agenda",
            &v(&[
                ("from", &["2026-11-01T00:00:00Z"]),
                ("to", &["2026-12-31T00:00:00Z"]),
            ]),
        )
        .unwrap();
    assert_eq!(ag.values["events"].len(), 5); // Meeting + 4 weekly instances
    assert!(ag.values["tasks"].is_empty());
}
