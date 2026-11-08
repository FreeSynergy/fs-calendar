//! `Calendar` over the socket of the calendar server (ADR 0063), split by responsibility.
//!
//! Pattern: Proxy — `proxy.rs` is the proxy, `worker.rs` the thread that owns the connection and
//! carries out the calls, `convert.rs` turns what the wire holds into the core's types.
//!
//! Provisional: only what the window needs (read, list, window, create, update, delete). Tasks are not
//! asked (`agenda` answers `Unsupported`, the window then shows events alone), invitations and
//! import/export go through the command line or the local mode.

mod convert;
mod proxy;
mod worker;

pub use proxy::RemoteCalendar;
