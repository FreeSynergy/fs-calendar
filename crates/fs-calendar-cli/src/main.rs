//! `fs-calendar`: wiring only.
//!
//! Pattern: none (thin binary) — parse, connect to the socket, run one command, print.

use fs_calendar_cli::{HELP, parse, run};
use fs_interfaces::events_capnp::events;
use fs_interfaces::rpc::{connect, default_socket};

fn main() {
    let words: Vec<String> = std::env::args().skip(1).collect();
    let args = match parse(&words) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("fs-calendar: {e}");
            std::process::exit(2);
        }
    };
    let Some(command) = args.command else {
        println!("{HELP}");
        return;
    };
    let Some(socket) = args.socket.or_else(|| default_socket("calendar")) else {
        eprintln!("fs-calendar: no socket: give --socket or set XDG_RUNTIME_DIR");
        std::process::exit(2);
    };
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!("fs-calendar: {e}");
            std::process::exit(1);
        }
    };
    let local = tokio::task::LocalSet::new();
    let out = local.block_on(&runtime, async move {
        let connection = connect::<events::Client>(&socket)
            .await
            .map_err(|e| format!("{}: {e}", socket.display()))?;
        run(&connection.client, command).await
    });
    match out {
        Ok(text) => println!("{text}"),
        Err(e) => {
            eprintln!("fs-calendar: {e}");
            std::process::exit(1);
        }
    }
}
