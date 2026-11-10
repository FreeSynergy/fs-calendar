//! Keys that name no instance, reads that race, and a change of the whole series after a range read.
//!
//! Pattern: none (tests).
#![allow(clippy::pedantic)]

mod common;

use common::*;
use fs_admin_frontend::{Backend, BackendError};
use fs_objects::ObjectId;

#[test]
fn a_key_that_is_no_instance_of_the_series_changes_and_deletes_nothing() {
    let w = world();
    let id = series_id(&w);
    let oid: ObjectId = id.parse().unwrap();
    // a Wednesday in a series that runs on Tuesdays, an unreadable key, a date long before the series
    for key in ["2026-11-25T10:00:00", "garbage", "1999-01-05T10:00:00", ""] {
        let at = format!("{id}#{key}");
        let change = w
            .backend
            .run("change", &v(&[("id", &[&at]), ("title", &["Extra"])]));
        assert_eq!(change, Err(BackendError::NotFound), "change {key}");
        let delete = w.backend.run("delete", &v(&[("id", &[&at])]));
        assert_eq!(delete, Err(BackendError::NotFound), "delete {key}");
    }
    let e = w.rt.block_on(w.repo.get_event(oid)).unwrap().unwrap();
    assert!(
        e.recurrence_overrides.is_empty(),
        "{:?}",
        e.recurrence_overrides
    );
    assert_eq!(w.backend.list(&range()).unwrap().len(), 4);
    // a key on an event that is no series
    let one = w
        .backend
        .run(
            "create",
            &v(&[
                ("title", &["One"]),
                ("start", &["2026-11-12T09:00:00"]),
                ("timeZone", &["UTC"]),
            ]),
        )
        .unwrap()
        .id
        .unwrap();
    let wrong = format!("{one}#2026-11-13T09:00:00");
    assert_eq!(
        w.backend.run("delete", &v(&[("id", &[&wrong])])),
        Err(BackendError::NotFound)
    );
    assert_eq!(w.backend.list(&v(&[])).unwrap().len(), 2);
    // the id a range read gives a single event (its own start as recurrence id) is the event itself
    let own = format!("{one}#2026-11-12T09:00:00");
    w.backend
        .run("change", &v(&[("id", &[&own]), ("title", &["Renamed"])]))
        .unwrap();
    let kept = w.backend.list(&v(&[])).unwrap();
    assert!(kept.iter().any(|r| r.values["title"] == ["Renamed"]));
    // an instance that was deleted is gone for a second change
    let real = format!("{id}#2026-12-01T10:00:00");
    w.backend.run("delete", &v(&[("id", &[&real])])).unwrap();
    assert_eq!(
        w.backend
            .run("change", &v(&[("id", &[&real]), ("title", &["x"])])),
        Err(BackendError::NotFound)
    );
}

#[test]
fn a_write_between_the_two_reads_of_a_range_is_a_conflict() {
    let w = world();
    let id = series_id(&w);
    let oid: ObjectId = id.parse().unwrap();
    *w.racing.retitle.lock().unwrap() = Some(oid);
    let shown = w.backend.list(&range()).unwrap();
    assert_eq!(shown[0].values["title"], ["Between the reads"]);
    match w.backend.run(
        "change",
        &v(&[("id", &[&shown[0].id]), ("title", &["Mine"])]),
    ) {
        Err(BackendError::Unavailable(m)) => assert!(m.starts_with("conflict"), "{m}"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_change_of_the_whole_series_after_a_range_read_conflicts() {
    let w = world();
    let id = series_id(&w);
    let oid: ObjectId = id.parse().unwrap();
    w.backend.list(&range()).unwrap();
    let mut e = w.rt.block_on(w.repo.get_event(oid)).unwrap().unwrap();
    e.title = Some("Other".into());
    w.rt.block_on(w.repo.update(oid, e)).unwrap();
    match w
        .backend
        .run("change", &v(&[("id", &[&id]), ("title", &["Mine"])]))
    {
        Err(BackendError::Unavailable(m)) => assert!(m.starts_with("conflict"), "{m}"),
        other => panic!("{other:?}"),
    }
    assert_eq!(
        w.rt.block_on(w.repo.get_event(oid))
            .unwrap()
            .unwrap()
            .title
            .as_deref(),
        Some("Other")
    );
}

#[test]
fn a_range_read_that_does_not_show_an_event_does_not_renew_its_revision() {
    let w = world();
    let oid: ObjectId = series_id(&w).parse().unwrap();
    // the form shows the event (normal list): this is the revision the person works on
    let shown = w.backend.list(&v(&[])).unwrap();
    // somebody else changes it
    let mut e = w.rt.block_on(w.repo.get_event(oid)).unwrap().unwrap();
    e.title = Some("Other".into());
    w.rt.block_on(w.repo.update(oid, e)).unwrap();
    // a range read that does not contain the event (0 hits)
    let far = v(&[
        ("from", &["2030-01-01T00:00:00Z"]),
        ("to", &["2030-02-01T00:00:00Z"]),
    ]);
    assert!(w.backend.list(&far).unwrap().is_empty());
    // the stale form is still a conflict, the other change survives
    let r = w.backend.run(
        "change",
        &v(&[("id", &[&shown[0].id]), ("title", &["Mine"])]),
    );
    assert!(matches!(r, Err(BackendError::Unavailable(m)) if m.starts_with("conflict")));
    assert_eq!(
        w.rt.block_on(w.repo.get_event(oid))
            .unwrap()
            .unwrap()
            .title
            .as_deref(),
        Some("Other")
    );
}

#[test]
fn a_range_read_whose_term_hides_an_event_does_not_renew_its_revision() {
    let w = world();
    let oid: ObjectId = series_id(&w).parse().unwrap();
    let shown = w.backend.list(&v(&[])).unwrap();
    let mut e = w.rt.block_on(w.repo.get_event(oid)).unwrap().unwrap();
    e.title = Some("Other".into());
    w.rt.block_on(w.repo.update(oid, e)).unwrap();
    // the range contains the event, the search term hides it (0 hits)
    let hidden = v(&[
        ("from", &["2026-11-01T00:00:00Z"]),
        ("to", &["2027-01-31T00:00:00Z"]),
        ("term", &["zzz"]),
    ]);
    assert!(w.backend.list(&hidden).unwrap().is_empty());
    let r = w.backend.run(
        "change",
        &v(&[("id", &[&shown[0].id]), ("title", &["Mine"])]),
    );
    assert!(matches!(r, Err(BackendError::Unavailable(m)) if m.starts_with("conflict")));
    assert_eq!(
        w.rt.block_on(w.repo.get_event(oid))
            .unwrap()
            .unwrap()
            .title
            .as_deref(),
        Some("Other")
    );
}
