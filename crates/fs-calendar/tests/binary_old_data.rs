//! The real program started on data written by v0.4.1 (no `page_key` in the index): it completes the
//! index at the start, so the list and a window lose nothing. The fixture is that of the core.

use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use fs_interfaces::events_capnp::events;
use fs_interfaces::rpc::connect;

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

#[tokio::test(flavor = "current_thread")]
async fn data_of_v0_4_1_is_listed_and_windowed_whole_after_the_start() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let dir = tempfile::tempdir().unwrap();
            let (socket, data) = (dir.path().join("events.sock"), dir.path().join("data"));
            std::fs::create_dir(&data).unwrap();
            std::fs::set_permissions(&data, std::fs::Permissions::from_mode(0o700)).unwrap();
            let from =
                Path::new(env!("CARGO_MANIFEST_DIR")).join("../fs-calendar-core/tests/fixtures");
            for (name, to) in [("v041.db", "objects.db"), ("v041.db-wal", "objects.db-wal")] {
                std::fs::copy(from.join(name), data.join(to)).unwrap();
            }
            let mut server = start(&socket, &data);
            for _ in 0..200 {
                if tokio::net::UnixStream::connect(&socket).await.is_ok() {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
            let connection = connect::<events::Client>(&socket).await.unwrap();
            let answer = connection
                .client
                .items_request()
                .send()
                .promise
                .await
                .unwrap();
            let items = answer.get().unwrap().get_items().unwrap();
            let listed = items.list_request().send().promise.await.unwrap();
            let page = listed.get().unwrap().get_page().unwrap();
            assert_eq!(page.get_items().unwrap().len(), 6);
            let mut window = items.occurrences_request();
            window.get().init_from().set_unix_millis(1_795_996_800_000);
            window.get().init_to().set_unix_millis(1_798_761_600_000);
            let got = window.send().promise.await.unwrap();
            assert_eq!(got.get().unwrap().get_items().unwrap().len(), 8);
            drop((answer, connection));
            let _ = server.kill();
            let _ = server.wait();
        })
        .await;
}
