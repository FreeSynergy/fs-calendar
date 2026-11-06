//! Invitations: module map.
//!
//! Pattern: State (declarations) — `status` holds the state machine, `flow` applies it to
//! stored events, `log` reads the history back.

mod flow;
mod log;
mod status;

pub use log::InvitationEntry;
pub use status::{ParticipationStatus, Response};
