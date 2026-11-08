//! `RemoteCalendar` against a real server and a real store, through the adapter the window uses.
//!
//! Pattern: Integration test — the server runs on its own thread and socket.

use std::path::Path;
use std::sync::Arc;

use chrono::{TimeZone, Utc};
use fs_calendar_app::RemoteCalendar;
use fs_calendar_core::{Calendar, CalendarRepo, Event};
use fs_calendar_server::CalendarServer;
use fs_interfaces::events_capnp::events;
use fs_interfaces::rpc::{bind_socket, serve};
use fs_objects::{Store, TursoStore};

/// A server on `socket`, on a thread of its own; it ends with the process.
fn start_server(socket: &Path) {
    let socket = socket.to_path_buf();
    let (up, started) = std::sync::mpsc::channel::<()>();
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        tokio::task::LocalSet::new().block_on(&runtime, async move {
            let store: Arc<dyn Store> = Arc::new(TursoStore::open_in_memory().await.unwrap());
            let repo = Arc::new(CalendarRepo::new(
                store,
                "https://local.invalid/users/owner",
            ));
            let root: events::Client = CalendarServer::new(repo.clone(), repo).into_client();
            let bound = bind_socket(&socket).await.unwrap();
            up.send(()).unwrap();
            serve(bound, root.client, std::future::pending()).await.ok();
        });
    });
    started.recv().unwrap();
}

#[test]
fn create_read_list_window_update_delete_over_the_socket() {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("events.sock");
    start_server(&socket);
    let remote = RemoteCalendar::connect(&socket).unwrap();
    let runtime = tokio::runtime::Runtime::new().unwrap();

    let made = runtime
        .block_on(remote.create(Event::new(
            "Talk",
            "2026-12-01T10:00:00",
            "Europe/Berlin",
            "PT1H",
        )))
        .unwrap();
    let id = made.envelope.id;
    assert_eq!(made.content["title"], "Talk");

    let again = runtime.block_on(remote.get(id)).unwrap().unwrap();
    assert_eq!(again.content["title"], "Talk");
    assert!(
        runtime
            .block_on(remote.get(fs_objects::ObjectId::new()))
            .unwrap()
            .is_none()
    );
    let uid = made.content["uid"].as_str().unwrap().to_string();
    assert!(runtime.block_on(remote.get_by_uid(&uid)).unwrap().is_some());
    assert_eq!(runtime.block_on(remote.list_events()).unwrap().len(), 1);

    let from = Utc.with_ymd_and_hms(2026, 12, 1, 0, 0, 0).unwrap();
    let to = Utc.with_ymd_and_hms(2026, 12, 2, 0, 0, 0).unwrap();
    let window = runtime.block_on(remote.window(from, to)).unwrap();
    assert_eq!(window.instances.len(), 1);
    assert_eq!(window.damaged.len(), 0);
    assert_eq!(window.instances[0].object, id);
    assert_eq!(
        window.instances[0].occurrence.start_utc,
        Utc.with_ymd_and_hms(2026, 12, 1, 9, 0, 0).unwrap(),
        "10:00 Berlin"
    );
    let elsewhere = runtime
        .block_on(remote.window(to, to + chrono::Duration::days(1)))
        .unwrap();
    assert_eq!(elsewhere.instances.len(), 0);

    let mut event = runtime.block_on(remote.get_event(id)).unwrap().unwrap();
    event.title = Some("Talk, moved".into());
    let updated = runtime.block_on(remote.update(id, event)).unwrap();
    assert_eq!(updated.content["title"], "Talk, moved");

    runtime.block_on(remote.delete(id)).unwrap();
    assert!(runtime.block_on(remote.get(id)).unwrap().is_none());
    assert_eq!(runtime.block_on(remote.list_events()).unwrap().len(), 0);
}

#[test]
fn no_server_is_a_message_that_says_what_to_start() {
    let dir = tempfile::tempdir().unwrap();
    let got = RemoteCalendar::connect(&dir.path().join("nope.sock"));
    let message = got.err().expect("nothing listens");
    assert!(message.contains("fs-calendar serve"), "{message}");
}
