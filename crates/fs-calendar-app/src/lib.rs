//! `fs-calendar-app`: the frontend of the calendar program (ADR 0005, 0009, 0037).
//!
//! Arguments, the connection to the servers, the words, and an adapter from the program's contract to the
//! workbench's `CalendarSource`. It imports no `blinc_*` crate: the window and the work surface come
//! from `fs-components`. The program talks to a running `fs-calendar-server` over its socket and
//! optionally to a `fs-tasks-server` for the tasks that are due; it holds no store (see `DESIGN.md`).
//!
//! Pattern: Facade — this file only declares modules and re-exports the public API.

#![deny(missing_docs)]
#![warn(clippy::pedantic)]
#![allow(clippy::must_use_candidate, clippy::module_name_repetitions)]

pub mod args;
mod import;
mod lost_zones;
mod marked_ics;
mod menus;
mod source;
mod surface;
mod transfer;
mod words;
mod zone;

pub use import::{ImportSummary, import_file};
pub use source::{CalendarAdapter, apply_draft, is_marked};
pub use surface::{CalendarScreen, app_widget, screen_widget};
pub use transfer::{ExportReport, export_all};
pub use words::{
    cannot_reach, labels, no_socket, no_tasks_socket, not_exported, tasks_cannot_reach, wrote,
};
pub use zone::{shown_zone, zone_from};
