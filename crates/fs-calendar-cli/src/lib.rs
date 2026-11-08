//! `fs-calendar`: the command line of the calendar.
//!
//! Pattern: Command — `args` turns words into a [`Command`] (pure), `run` carries one out against an
//! `events::Client` and answers the text to print. The client is whatever the caller connected: the
//! binary connects to the socket, the tests to a server in the same process. Nothing here touches
//! the store; the CLI is a second user of `events.capnp` (the first is the MCP tools).

pub mod args;
mod program;
mod run;
mod show;

pub use args::{Command, HELP, parse};
pub use program::cli_command;
pub use run::run;
