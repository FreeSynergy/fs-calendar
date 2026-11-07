//! The Cap'n Proto contract `events.capnp` in fs-interfaces (draft) and its conversions: module map.
//!
//! Pattern: Adapter (declarations) — the generated code (`events_capnp`, `common_capnp` at the crate
//! root, committed, ADR 0040) is wrapped by `convert`, which maps wire types to the Rust interface
//! types. Interface status: draft (ADR 0045).

mod convert;

pub use convert::{response_from_wire, response_to_wire};

/// Version of the contract in `events.capnp`, as written in its header.
pub const CONTRACT_VERSION: &str = "events 0.1 (draft)";
