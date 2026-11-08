//! The real program: started, used, stopped and started again; the events are still there.

use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use fs_interfaces::events_capnp::events;
use fs_interfaces::rpc::{connect, read_envelope};

fn start(socket: &Path, data: &Path) -> Child {
    Command::new(env!("CARGO_BIN_EXE_fs-calendar"))
        .arg("serve")
        .arg("--socket")
        .arg(socket)
        .arg("--data")
        .arg(data)
        .stderr(Stdio::null())
        .spawn()
        .unwrap()
}

async fn wait_for(socket: &Path) {
    for _ in 0..200 {
        if tokio::net::UnixStream::connect(socket).await.is_ok() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("the server did not come up");
}

fn terminate(mut child: Child) {
    let status = Command::new("kill")
        .arg("-TERM")
        .arg(child.id().to_string())
        .status()
        .unwrap();
    assert!(status.success());
    assert!(child.wait().unwrap().success());
}

async fn stored_events(socket: &Path) -> usize {
    let connection = connect::<events::Client>(socket).await.unwrap();
    let answer = connection
        .client
        .items_request()
        .send()
        .promise
        .await
        .unwrap();
    let items = answer.get().unwrap().get_items().unwrap();
    let listed = items.list_request().send().promise.await.unwrap();
    listed
        .get()
        .unwrap()
        .get_page()
        .unwrap()
        .get_items()
        .unwrap()
        .len() as usize
}

#[tokio::test(flavor = "current_thread")]
async fn events_survive_a_restart_and_a_second_start_is_refused() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let dir = tempfile::tempdir().unwrap();
            let socket = dir.path().join("events.sock");
            let data = dir.path().join("data");

            let first = start(&socket, &data);
            wait_for(&socket).await;
            let mode = std::fs::metadata(&data).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o700, "the data is for this user alone");

            let mut second = start(&socket, &data);
            assert_eq!(
                second.wait().unwrap().code(),
                Some(1),
                "a second server started"
            );
            assert!(socket.exists(), "the second start touched the first");

            let connection = connect::<events::Client>(&socket).await.unwrap();
            let answer = connection
                .client
                .items_request()
                .send()
                .promise
                .await
                .unwrap();
            let items = answer.get().unwrap().get_items().unwrap();
            let mut create = items.create_request();
            create
                .get()
                .set_jscalendar(r#"{"@type":"Event","version":"2.0","uid":"kept","title":"Kept","start":"2026-12-01T10:00:00"}"#);
            create.send().promise.await.unwrap();
            drop(answer);
            drop(connection);

            terminate(first);
            assert!(!socket.exists(), "the socket stayed behind");

            let again = start(&socket, &data);
            wait_for(&socket).await;
            assert_eq!(stored_events(&socket).await, 1);
            terminate(again);
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn the_instance_names_the_iri_of_a_new_event() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let dir = tempfile::tempdir().unwrap();
            let socket = dir.path().join("events.sock");
            let server = Command::new(env!("CARGO_BIN_EXE_fs-calendar"))
        .arg("serve")
                .arg("--socket")
                .arg(&socket)
                .arg("--data")
                .arg(dir.path().join("data"))
                .args(["--instance", "calendar.example.org"])
                .stderr(Stdio::null())
                .spawn()
                .unwrap();
            wait_for(&socket).await;
            let connection = connect::<events::Client>(&socket).await.unwrap();
            let answer = connection
                .client
                .items_request()
                .send()
                .promise
                .await
                .unwrap();
            let items = answer.get().unwrap().get_items().unwrap();
            let mut create = items.create_request();
            create
                .get()
                .set_jscalendar(r#"{"@type":"Event","version":"2.0","uid":"i","title":"I","start":"2026-12-01T10:00:00"}"#);
            let created = create.send().promise.await.unwrap();
            let event = created.get().unwrap().get_event().unwrap();
            let envelope = read_envelope(event.get_envelope().unwrap()).unwrap();
            let iri = envelope.iri.unwrap();
            drop(created);
            drop(answer);
            drop(connection);
            // Stopped first: a failed assertion must not leave the server running.
            terminate(server);
            assert!(iri.starts_with("https://calendar.example.org/events/"), "{iri}");
        })
        .await;
}
