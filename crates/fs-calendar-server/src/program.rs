//! The server as a command: what `fs-calendar serve` runs.
//!
//! Pattern: Command (one subcommand of the single `fs-calendar` program, ADR 0063) — reads its options,
//! opens the store, serves until a signal. Tasks of an imported file are not stored here: no task
//! program is attached, and the import report says so. Returns the exit code.

use std::ffi::OsString;
use std::sync::Arc;

use fs_calendar_core::CalendarRepo;
use fs_interfaces::rpc::{ServerOptions, prepare_data_dir, run_until_signal};
use fs_objects::{Store, TursoStore};

use crate::CalendarServer;

const USAGE: &str = "fs-calendar serve [--socket PATH] [--data DIR] [--actor IRI] [--instance DOMAIN] [--lease FILE]\n--lease FILE: end when no window holds this lock any more (started by a window, not by hand)\nDefaults: $XDG_RUNTIME_DIR/freesynergy/calendar.sock, $XDG_DATA_HOME/freesynergy/calendar, the local owner";

/// Run the server with `args` (the words after `serve`); 0 after a signal or `--help`, 2 for a mistake
/// in the arguments, 1 when serving failed.
#[must_use]
pub fn serve_command(args: impl IntoIterator<Item = OsString>) -> i32 {
    let (lease, args) = match crate::lease::take_lease(args.into_iter().collect()) {
        Ok(taken) => taken,
        Err(problem) => {
            eprintln!("fs-calendar serve: {problem}");
            return 2;
        }
    };
    let options = match ServerOptions::parse("calendar", args) {
        Ok(Some(options)) => options,
        Ok(None) => {
            println!("{USAGE}");
            return 0;
        }
        Err(problem) => {
            eprintln!("fs-calendar serve: {problem}");
            return 2;
        }
    };
    let ServerOptions {
        socket,
        data,
        actor,
        instance,
    } = options;
    if let Some(lease) = lease {
        // The window that started this server holds the lease; when none does, end as a signal would.
        crate::lease::watch(lease, || {
            let _ = rustix::process::kill_process(
                rustix::process::getpid(),
                rustix::process::Signal::TERM,
            );
        });
    }
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
                "fs-calendar serve: index completed for {} events",
                report.rewritten
            );
        }
        for failed in &report.failed {
            eprintln!("fs-calendar serve: event not repaired: {failed}");
        }
        let repo = Arc::new(repo);
        Ok(CalendarServer::new(repo.clone(), repo).into_client().client)
    });
    match served {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("fs-calendar serve: {e}");
            1
        }
    }
}
