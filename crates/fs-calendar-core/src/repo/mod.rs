//! The store-backed calendar: module map.
//!
//! Pattern: Facade (declarations) — `CalendarRepo` is the one entry point; the files behind it
//! are one concept each (build, CRUD, update, queries, migration, import / export).

mod agenda;
mod backend;
mod book;
mod build;
mod consts;
mod crud;
mod handle;
mod migrate;
mod page;
mod queries;
mod span;
mod transfer;
mod types;
mod update;

pub use consts::{CONTENT_TYPE, CONTEXT, EVENT_TYPE};
pub use handle::CalendarRepo;
pub use migrate::IndexReport;
pub use page::EventPage;
pub use types::{Agenda, Damaged, ImportReport, Instance, Window};
