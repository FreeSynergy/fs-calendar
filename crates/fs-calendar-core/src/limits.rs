//! The limits of the calendar, in one place.
//!
//! Pattern: Registry (constants) — every bound the core enforces, so the server and the tests name
//! the same numbers.

/// Largest event: its JSON plus the name in its envelope, as an answer carries both and the title
/// counts twice. With the rest of the envelope that fits a page of the contract
/// (`fs_interfaces::rpc::MAX_INTAKE_BYTES`, half of `MAX_PAGE_BYTES`).
pub const MAX_EVENT_BYTES: usize = 16 * 1024 * 1024;

/// Most instances of one event that one question returns; the rest is reported as cut.
pub const MAX_INSTANCES_PER_EVENT: usize = 10_000;

/// Most steps of a rule the engine may walk from the series start to the end of the window. `rrule`
/// walks from `DTSTART`, so a daily series asked about year 8999 costs seconds. Beyond this the
/// question is refused for that event and reported, instead of freezing every caller.
pub const MAX_RULE_STEPS: u64 = 500_000;
