//! `fs-calendar-server`: the contract `events.capnp` over a Unix socket, in front of any provider of
//! `Calendar` and `CalendarFeed`. It only translates: no calendar logic lives here.
//!
//! Interface status: draft (ADR 0045).
//!
//! Pattern: Facade — this file only declares modules and re-exports.

#![deny(missing_docs)]
#![warn(clippy::pedantic)]
// The generated `Server` traits are async for every method, also those that never wait.
#![allow(clippy::missing_errors_doc, clippy::unused_async_trait_impl)]

mod errors;
mod events_service;
mod feed;
mod invitations_service;
mod items_service;
mod report;
mod wire;

pub use events_service::{CalendarServer, MAX_CONCURRENT_WRITES};
