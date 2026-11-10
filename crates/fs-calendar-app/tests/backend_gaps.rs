//! The agenda with tasks, and the refusals of respond and of input.
//!
//! Pattern: none (tests).
#![allow(clippy::pedantic)]

mod common;

use common::*;
use fs_admin_frontend::{Backend, BackendError};
use fs_tasks_core::{Task, TaskItems};

#[test]
fn the_agenda_has_events_with_their_instance_ids_and_the_tasks_due() {
    let w = world();
    let mut task = Task::new("Send the minutes");
    task.due = Some("2026-12-02T17:00:00".to_owned());
    task.time_zone = Some("Europe/Berlin".to_owned());
    let task_id =
        w.rt.block_on(TaskItems::create(w.tasks.as_ref(), task))
            .unwrap()
            .id;
    let single = w
        .backend
        .run(
            "create",
            &v(&[
                ("title", &["One"]),
                ("start", &["2026-12-02T09:00:00"]),
                ("timeZone", &["UTC"]),
            ]),
        )
        .unwrap()
        .id
        .unwrap();
    let a = w
        .backend
        .run(
            "agenda",
            &v(&[
                ("from", &["2026-11-30T00:00:00Z"]),
                ("to", &["2026-12-10T00:00:00Z"]),
            ]),
        )
        .unwrap();
    let events = &a.values["events"];
    // every instance is `<id>#<recurrence id>`; the recurrence id of a single event is its own start, and that
    // id names the event itself for change and delete (`backend_keys.rs`)
    assert_eq!(events.len(), 3, "{events:?}");
    assert!(events.iter().all(|e| e.contains('#')));
    assert!(events.contains(&format!("{single}#2026-12-02T09:00:00")));
    assert!(events.iter().any(|e| e.ends_with("#2026-12-01T10:00:00")));
    assert_eq!(a.values["tasks"], [format!("urn:uuid:{task_id}")]);
    // outside the range there is no task
    let none = w
        .backend
        .run(
            "agenda",
            &v(&[
                ("from", &["2027-03-01T00:00:00Z"]),
                ("to", &["2027-03-02T00:00:00Z"]),
            ]),
        )
        .unwrap();
    assert!(none.values["tasks"].is_empty());
}

fn invited(w: &World) -> String {
    let id = series_id(w);
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
    id
}

fn entries(w: &World, id: &str) -> usize {
    w.backend
        .run("history", &v(&[("id", &[id])]))
        .unwrap()
        .values["entries"]
        .len()
}

#[test]
fn respond_refuses_a_capital_answer_and_writes_nothing() {
    let w = world();
    let id = invited(&w);
    let r = w.backend.run(
        "respond",
        &v(&[
            ("id", &[&id]),
            ("who", &["ben@example.org"]),
            ("response", &["Accept"]),
        ]),
    );
    assert!(matches!(r, Err(BackendError::Rejected(i)) if i[0].field == "response"));
    assert_eq!(entries(&w, &id), 1);
}

#[test]
fn respond_accepts_each_known_answer() {
    let w = world();
    let id = invited(&w);
    for (n, answer) in ["accept", "tentative-accept", "reject"].iter().enumerate() {
        w.backend
            .run(
                "respond",
                &v(&[
                    ("id", &[&id]),
                    ("who", &["ben@example.org"]),
                    ("response", &[answer]),
                ]),
            )
            .unwrap();
        assert_eq!(entries(&w, &id), 2 + n, "{answer}");
    }
}

#[test]
fn unknown_input_fields_are_refused_by_change_and_create() {
    let w = world();
    let id = series_id(&w);
    let r = w
        .backend
        .run("change", &v(&[("id", &[&id]), ("titel", &["x"])]));
    assert!(matches!(r, Err(BackendError::Rejected(i)) if i[0].field == "titel"));
    let r = w.backend.run(
        "create",
        &v(&[("start", &["2026-11-12T09:00:00"]), ("id", &["x"])]),
    );
    assert!(matches!(r, Err(BackendError::Rejected(i)) if i[0].field == "id"));
}
