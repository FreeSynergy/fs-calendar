//! `fs-calendar-app`: the frontend of the calendar program (ADR 0005, 0009, 0037).
//!
//! Arguments, opening the core, the words, and an adapter from the program's contract to the
//! workbench's `CalendarSource`. It imports no `blinc_*` crate: the window and the work surface come
//! from `fs-components`. The core is called in process for now, not over RPC (see `DESIGN.md`).
//!
//! Pattern: Facade — this file only declares modules and re-exports the public API.

#![deny(missing_docs)]
#![warn(clippy::pedantic)]
#![allow(clippy::must_use_candidate, clippy::module_name_repetitions)]

pub mod args;
mod source;
mod surface;
mod transfer;
mod words;
mod zone;

pub use source::{CalendarAdapter, apply_draft, is_marked};
pub use surface::{CalendarScreen, app_widget, screen_widget};
pub use transfer::{ExportReport, ImportSummary, export_all, import_file};
pub use words::labels;
pub use zone::shown_zone;
