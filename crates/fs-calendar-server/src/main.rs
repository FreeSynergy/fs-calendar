//! `fs-calendar-server`. Usage: `fs-calendar-server [--socket PATH] [--data DIR] [--actor IRI] [--instance DOMAIN]`.
//!
//! Pattern: none (thin binary) — reads its options, opens the store, serves until a signal. Tasks of an
//! imported file are not stored here: no task program is attached, and the import report says so.

use std::sync::Arc;

use fs_calendar_core::CalendarRepo;
use fs_calendar_server::CalendarServer;
use fs_interfaces::rpc::{ServerOptions, prepare_data_dir, run_until_signal};
use fs_store::{Store, TursoStore};

fn main() {
    let options = ServerOptions::from_env_or_exit("calendar");
    let ServerOptions {
        socket,
        data,
        actor,
        instance,
    } = options;
    let served = run_until_signal(&socket, || async move {
        let data = prepare_data_dir(&data)?;
        let store = TursoStore::open(data.join("objects.db"))
            .await
            .map_err(std::io::Error::other)?;
        let store: Arc<dyn Store> = Arc::new(store);
        let mut repo = CalendarRepo::new(store, actor);
        if let Some(instance) = instance {
            repo = repo.with_instance(instance);
        }
        // Events stored before v0.5.0 get the index field the pages and windows need; what cannot be
        // repaired is named, and a page or a window says so as well.
        let report = repo.upgrade_index().await.map_err(std::io::Error::other)?;
        if report.rewritten > 0 {
            eprintln!(
                "fs-calendar-server: index completed for {} events",
                report.rewritten
            );
        }
        for failed in &report.failed {
            eprintln!("fs-calendar-server: event not repaired: {failed}");
        }
        let repo = Arc::new(repo);
        Ok(CalendarServer::new(repo.clone(), repo).into_client().client)
    });
    if let Err(e) = served {
        eprintln!("fs-calendar-server: {e}");
        std::process::exit(1);
    }
}
