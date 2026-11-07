//! The real binary against the real server: it connects by `--socket`, and without one it says how.
//!
//! Integration tests of the process; no window is opened (export, or a picture).

mod common;

use std::process::Command;

use common::{Scratch, World, rich};

fn app() -> Command {
    Command::new(env!("CARGO_BIN_EXE_fs-calendar-app"))
}

#[test]
fn without_a_socket_it_stops_and_names_the_server() {
    let out = app().output().unwrap();
    assert!(!out.status.success());
    let said = String::from_utf8_lossy(&out.stderr);
    assert!(said.contains("fs-calendar-server --socket"), "{said}");
}

#[test]
fn an_own_store_is_refused_and_the_server_is_named() {
    let dir = Scratch::new();
    let out = app().arg("--store").arg(dir.file("a.db")).output().unwrap();
    assert!(!out.status.success());
    let said = String::from_utf8_lossy(&out.stderr);
    assert!(said.contains("fs-calendar-server --data"), "{said}");
    assert!(!dir.file("a.db").exists(), "no store was made");
}

#[test]
fn a_socket_nobody_listens_on_is_an_error_in_words() {
    let dir = Scratch::new();
    let out = app()
        .arg("--socket")
        .arg(dir.file("none.sock"))
        .output()
        .unwrap();
    assert!(!out.status.success());
    let said = String::from_utf8_lossy(&out.stderr);
    assert!(said.contains("cannot reach the calendar server"), "{said}");
}

#[test]
fn a_tasks_socket_nobody_listens_on_is_an_error_in_words() {
    let w = World::berlin();
    let dir = Scratch::new();
    let out = app()
        .arg("--socket")
        .arg(&w.socket)
        .arg("--tasks-socket")
        .arg(dir.file("none.sock"))
        .output()
        .unwrap();
    assert!(!out.status.success());
    let said = String::from_utf8_lossy(&out.stderr);
    assert!(said.contains("cannot reach the tasks server"), "{said}");
}

#[test]
fn it_imports_and_exports_through_the_server_and_says_it_shows_no_tasks_otherwise() {
    let w = World::berlin();
    w.add_json(rich());
    let dir = Scratch::new();
    let (out_file, ics) = (dir.file("out.ics"), dir.file("in.ics"));
    std::fs::write(
        &ics,
        "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//t//EN\r\nBEGIN:VEVENT\r\nUID:file-1\r\nDTSTAMP:20261101T000000Z\r\nDTSTART:20261203T100000Z\r\nDTEND:20261203T110000Z\r\nSUMMARY:From a file\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n",
    )
    .unwrap();
    let out = app()
        .arg("--socket")
        .arg(&w.socket)
        .arg("--import")
        .arg(&ics)
        .arg("--export")
        .arg(&out_file)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = std::fs::read_to_string(&out_file).unwrap();
    assert!(
        text.contains(":Kick-off") && text.contains("SUMMARY:From a file"),
        "{text}"
    );
    let stored = w
        .runtime
        .block_on(fs_calendar_core::Calendar::list_events(w.repo.as_ref()))
        .unwrap();
    assert_eq!(stored.len(), 2, "the server holds both");
}

#[test]
fn a_calendar_without_a_tasks_socket_says_so() {
    let w = World::berlin();
    let dir = Scratch::new();
    let picture = dir.file("p.png");
    let out = app()
        .arg("--socket")
        .arg(&w.socket)
        .arg("--date")
        .arg("2026-11-06")
        .arg("--snapshot")
        .arg(&picture)
        .output()
        .unwrap();
    let said = String::from_utf8_lossy(&out.stderr);
    assert!(
        said.contains("No --tasks-socket given, so no tasks are shown"),
        "{said}"
    );
}
