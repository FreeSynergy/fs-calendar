//! The description of an event: what a generated frontend is told (ADR 0066).
//!
//! Pattern: Registry entry (trait first, ADR 0043) — [`Event`](crate::Event) implements `DescribesType` from
//! `fs-types` (`event.rs`), so `TypeRegistry::register::<Event>()` puts it in the catalog the program serves. Fields
//! come from the table in `fields.rs`, commands from `commands.rs` and the support table (`crate::support`).

mod commands;
mod event;
mod fields;

pub use event::{EVENT, PROGRAM, STANDARD, kebab};
pub use fields::{FREE_BUSY, PRIORITY, PRIVACY, STATUS, YES_NO};

pub(crate) use event::describe_field;
