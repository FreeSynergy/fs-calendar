//! `fs-calendar-app`: the calendar program's window (ADR 0065, 0066).
//!
//! The Event description of the core, an `EventBackend` over the core's own interface, and the window: the shell with
//! the calendar grid and the generated admin screen. It draws nothing itself (no `blinc_*` here): the components are
//! `fs-components`.
//!
//! Pattern: Facade — this file only declares modules and re-exports the public API.

#![deny(missing_docs)]
#![warn(clippy::pedantic)]
#![allow(clippy::must_use_candidate, clippy::module_name_repetitions)]

pub mod args;
mod backend;
pub mod frontend_types;
mod grid_source;
mod grid_surface;
mod program;
mod window;
mod words;
mod zone;

pub use backend::{EventBackend, apply, instance_id, iri, to_values};
pub use grid_source::CalendarSource;
pub use grid_surface::GridSurface;
pub use program::app_command;
pub use window::{CALENDAR, EVENT, calendar_window};
pub use words::Words;
pub use zone::{shown_zone, zone_from};
