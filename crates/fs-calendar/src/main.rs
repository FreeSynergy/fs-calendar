//! `fs-calendar`: the one program of FreeSynergy.Calendar (ADR 0063).
//!
//! Pattern: Command — the first word picks the subcommand and each one is a library of its own:
//! `serve` (`fs-calendar-server`), `app` or no word at all (`fs-calendar-app`, feature `gui`), anything else
//! is a command of the command line (`fs-calendar-cli`). Without the feature `gui` the bare command
//! prints the help, so a server does not hang on graphics libraries.

use std::ffi::OsString;

fn main() {
    let words: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(dispatch(&words));
}

fn dispatch(words: &[String]) -> i32 {
    match words.split_first() {
        Some((first, rest)) if first == "serve" => {
            fs_calendar_server::serve_command(rest.iter().map(OsString::from))
        }
        Some((first, rest)) if first == "app" => window(rest),
        None => window(&[]),
        Some(_) => fs_calendar_cli::cli_command(words),
    }
}

#[cfg(feature = "gui")]
fn window(args: &[String]) -> i32 {
    fs_calendar_app::app_command(args)
}

#[cfg(not(feature = "gui"))]
fn window(args: &[String]) -> i32 {
    if args.is_empty() {
        println!("{}", fs_calendar_cli::HELP);
        0
    } else {
        eprintln!("fs-calendar: this build has no window (feature gui)");
        2
    }
}
