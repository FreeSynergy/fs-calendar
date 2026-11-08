//! The commands against a real server on a real socket (in this process).

use std::sync::Arc;

use fs_calendar_cli::{Command, run};
use fs_calendar_core::CalendarRepo;
use fs_calendar_server::CalendarServer;
use fs_interfaces::events_capnp::events;
use fs_interfaces::rpc::{bind_socket, connect, serve};
use fs_objects::{Store, TursoStore};

async fn with_cli<F, Fut>(test: F)
where
    F: FnOnce(events::Client) -> Fut,
    Fut: Future<Output = ()>,
{
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let store: Arc<dyn Store> = Arc::new(TursoStore::open_in_memory().await.unwrap());
            let repo = Arc::new(CalendarRepo::new(
                store,
                "https://local.invalid/users/owner",
            ));
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("events.sock");
            let root = CalendarServer::new(repo.clone(), repo).into_client();
            let bound = bind_socket(&path).await.unwrap();
            let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
            let serving = tokio::task::spawn_local(serve(bound, root.client, async {
                let _ = stopped.await;
            }));
            let connection = connect::<events::Client>(&path).await.unwrap();
            test(connection.client.clone()).await;
            drop(connection);
            let _ = stop.send(());
            let _ = serving.await;
        })
        .await;
}

fn add(title: &str) -> Command {
    Command::Add {
        title: title.into(),
        start: "2026-12-01T10:00:00".into(),
        zone: Some("Europe/Berlin".into()),
        duration: None,
    }
}

#[tokio::test(flavor = "current_thread")]
async fn add_list_show_export_delete() {
    with_cli(|root| async move {
        let added = run(&root, add("Talk")).await.unwrap();
        let id = added.split("  ").next().unwrap().to_string();
        assert!(added.ends_with("  2026-12-01T10:00:00  Talk"), "{added}");

        assert_eq!(run(&root, Command::List).await.unwrap(), added);
        let shown = run(&root, Command::Show(id.clone())).await.unwrap();
        assert!(shown.contains("\"title\": \"Talk\""), "{shown}");

        let ics = run(&root, Command::Export).await.unwrap();
        assert!(
            ics.contains("BEGIN:VEVENT") && ics.contains("SUMMARY:Talk"),
            "{ics}"
        );

        assert_eq!(
            run(&root, Command::Delete(id.clone())).await.unwrap(),
            format!("deleted {id}")
        );
        assert_eq!(run(&root, Command::List).await.unwrap(), "");
        assert!(run(&root, Command::Show(id)).await.is_err());
        assert!(run(&root, Command::Show("not-an-id".into())).await.is_err());
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn import_reports_created_then_skipped() {
    with_cli(|root| async move {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("in.ics");
        std::fs::write(
            &file,
            "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//x//y//EN\r\nBEGIN:VEVENT\r\nUID:ev-1\r\nDTSTAMP:20261101T000000Z\r\nDTSTART:20261201T100000Z\r\nDURATION:PT1H\r\nSUMMARY:Standup\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n",
        )
        .unwrap();
        assert_eq!(
            run(&root, Command::Import(file.clone())).await.unwrap(),
            "created 1, skipped 0, failed 0"
        );
        assert_eq!(
            run(&root, Command::Import(file)).await.unwrap(),
            "created 0, skipped 1, failed 0"
        );
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn an_import_file_that_cannot_be_read_is_an_error_naming_it() {
    with_cli(|root| async move {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("nope.txt");
        let got = run(&root, Command::Import(missing.clone())).await;
        assert!(got.unwrap_err().contains("nope.txt"));
        let binary = dir.path().join("bin.dat");
        std::fs::write(&binary, [0xff, 0xfe, 0x00]).unwrap();
        let got = run(&root, Command::Import(binary)).await;
        assert!(got.unwrap_err().contains("not UTF-8"));
    })
    .await;
}
