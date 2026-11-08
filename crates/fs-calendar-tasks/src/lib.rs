//! `fs-calendar-tasks`: the task program as the calendar sees it (ADR 0050).
//!
//! The core of the calendar depends on no task crate; it asks for tasks through the two small traits
//! `TasksInWindow` and `TaskIntake`. This crate puts them in front of the contract `tasks.capnp`:
//! [`RpcTasks`] talks to `fs-tasks-server` over its Unix socket and knows the contract only, not the
//! types of `fs-tasks-core`. With the feature `local`, `LocalTasks` puts the same traits in front of
//! a `TaskItems` held in this process.
//!
//! Pattern: Facade — this file only declares modules and re-exports the public API.

#![deny(missing_docs)]
#![warn(clippy::pedantic)]
#![allow(
    clippy::missing_errors_doc,
    clippy::must_use_candidate,
    clippy::redundant_closure_for_method_calls
)]

mod calls;
#[cfg(feature = "local")]
mod local;
mod problems;
mod remote;
mod rpc_tasks;
mod traits_impl;

#[cfg(feature = "local")]
pub use local::LocalTasks;
pub use rpc_tasks::RpcTasks;
