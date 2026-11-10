//! `EventBackend`: the records and commands of the `Event` description over the calendar's own interface.
//!
//! Pattern: Adapter (`Backend` of `fs-admin-frontend` over the `Calendar` trait, ADR 0043) — it depends on the trait,
//! never on `CalendarRepo`, so the same code serves an in-process repository and a fake. The frontend is synchronous,
//! the core is not: calls run on the runtime of the program (`Handle::block_on`, the window thread is not part of it).
//! Revisions read by `list` are kept, so a `change` is an `updateIf` against what the person saw.

mod commands;
mod errors;
mod find;
mod objects;
mod occurrence;
mod record;
mod transfer;

mod event_backend;

pub use event_backend::{EventBackend, instance_id, iri};
pub use record::{apply, to_values};
