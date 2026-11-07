//! `fs-calendar-client`: the calendar program as a client of its own contract `events.capnp`, and the
//! tasks of a day as a client of `tasks.capnp`.
//!
//! [`RpcCalendar`] implements [`fs_calendar_core::Calendar`] over a Unix socket, so a frontend depends
//! on the trait and talks to a running `fs-calendar-server`, never to the core in its own process
//! (ADR 0021). What a method does not offer answers the `Unsupported` of the provider rule (ADR 0045);
//! `DESIGN.md` lists which. [`TaskLines`] is the narrow trait with which the calendar reads the tasks
//! due, [`RpcTaskLines`] its implementation over the tasks server: the calendar knows no code of the
//! tasks program (ADR 0050).
//!
//! Interface status: draft (ADR 0045), as the contracts.
//!
//! Pattern: none — this file only declares modules and re-exports the public API (no behaviour).

#![deny(missing_docs)]
#![warn(clippy::pedantic)]
// A closure reads better than the full path of a generated reader type.
#![allow(
    clippy::module_name_repetitions,
    clippy::redundant_closure_for_method_calls
)]

mod calendar_impl;
mod calls;
mod detail;
mod offered;
mod peer;
mod problems;
#[cfg(test)]
mod problems_table;
mod read;
mod remote;
mod revised;
mod revised_impl;
mod rpc_tasks;
mod task_lines;
mod tasks_remote;

pub use detail::changed_elsewhere;
pub use fs_interfaces::rpc::client::{CALL_TIMEOUT, CONNECT_TIMEOUT, Timeouts};
pub use peer::RpcCalendar;
pub use revised::{Revised, RevisedCalendar};
pub use rpc_tasks::RpcTaskLines;
pub use task_lines::{TaskLine, TaskLines, TasksUnread};
