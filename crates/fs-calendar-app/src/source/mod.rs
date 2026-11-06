//! From the program's contract to the workbench's source.
//!
//! Pattern: Adapter — `adapter.rs` drives the async contract from the interface thread, `key.rs` names
//! what the calendar shows, `items.rs` and `when.rs` turn instances into wall-clock items, `sheet.rs`
//! shows an event, `draft.rs` makes a form of it, `apply.rs` writes a form back onto it.

mod adapter;
mod apply;
mod apply_clock;
mod clash;
mod draft;
mod early;
mod found;
mod items;
mod items_read;
mod key;
mod location;
mod marks;
mod provider;
mod sheet;
mod task_sheet;
mod when;

pub use adapter::CalendarAdapter;
pub(crate) use adapter::reason_text;
pub use apply::apply_draft;
pub(crate) use found::caught;
pub(crate) use marks::SENSITIVE_KEY;
pub use marks::is_marked;
