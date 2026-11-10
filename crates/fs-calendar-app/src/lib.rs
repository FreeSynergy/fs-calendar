//! `fs-calendar-app`: the calendar program's window (ADR 0065, 0066).
//!
//! The Event description of the core, an `EventBackend` over the core's own interface, and the window the generated
//! admin frontend and the calendar grid component make. It draws nothing itself (no `blinc_*` here): the
//! components are `fs-components`.
//!
//! Pattern: Facade — this file only declares modules and re-exports the public API.

#![deny(missing_docs)]
#![warn(clippy::pedantic)]
#![allow(clippy::must_use_candidate, clippy::module_name_repetitions)]

mod backend;

pub use backend::{EventBackend, apply, iri, to_values};
