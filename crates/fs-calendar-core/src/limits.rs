//! The limits of the calendar, in one place.
//!
//! Pattern: Constants (coding.md section 2: a file of constants is not a Registry) — every bound the
//! core enforces, so the server and the tests name the same numbers.

/// Largest event: its JSON plus the name in its envelope, as an answer carries both and the title
/// counts twice. With the rest of the envelope that fits a page of the contract
/// (`fs_interfaces::rpc::MAX_INTAKE_BYTES`, half of `MAX_PAGE_BYTES`).
pub const MAX_EVENT_BYTES: usize = 16 * 1024 * 1024;

/// Most values (nodes) and most levels of one event; both are checked while the text is read.
pub const MAX_EVENT_NODES: usize = 100_000;
/// See [`MAX_EVENT_NODES`].
pub const MAX_EVENT_DEPTH: usize = 32;

/// Most memory one event may need, estimated by `model::memory_weight` (32 bytes a node, 640 bytes an
/// object, tested to be above what a copy really takes). A question holds about a dozen copies of one
/// event at once (store row, value, event, master, instance); the measured bound is in `DESIGN.md`.
pub const MAX_EVENT_MEMORY_BYTES: usize = 24 * 1024 * 1024;

/// Events read from the store at once for a window: the first page, then as many as fit
/// [`PAGE_WEIGHT_BYTES`] by the heaviest of the page before, at most [`MAX_PAGE_EVENTS`].
pub const FIRST_PAGE_EVENTS: usize = 4;
/// See [`FIRST_PAGE_EVENTS`].
pub const PAGE_WEIGHT_BYTES: usize = 16 * 1024 * 1024;
/// See [`FIRST_PAGE_EVENTS`].
pub const MAX_PAGE_EVENTS: usize = 256;

/// Most instances of one event that one question returns; the rest is reported as cut.
pub const MAX_INSTANCES_PER_EVENT: usize = 10_000;

/// First and last year of a start. The time library saturates at the edges of years 0 and 9999, so a
/// start closer to them could silently mean another time once converted to UTC: it is refused instead.
pub const FIRST_YEAR: i32 = 1;
/// See [`FIRST_YEAR`].
pub const LAST_YEAR: i32 = 9998;

/// Most bytes of instances one question may build: a page of the contract (32 MiB) and a little more.
/// An instance is counted as the JSON of its event (rule and overrides included, so an upper bound of
/// what it really weighs) plus [`INSTANCE_OVERHEAD_BYTES`]; the server counts the same overhead. When the
/// count passes this, the instances over it are not built and the cut is reported (`Damaged::cut`);
/// the server answers that with `too-large` instead of a shorter answer.
pub const MAX_ANSWER_BYTES: usize = 33 * 1024 * 1024;

/// What an instance weighs beyond the JSON of its event (the occurrence, ids, times).
pub const INSTANCE_OVERHEAD_BYTES: usize = 256;

/// `now` plus `days`, at the edge of the time type if that is further than it holds (a negative
/// number of days gives `now`: a window never ends before it starts).
pub fn after_days(now: chrono::DateTime<chrono::Utc>, days: i64) -> chrono::DateTime<chrono::Utc> {
    chrono::Duration::try_days(days.max(0))
        .and_then(|d| now.checked_add_signed(d))
        .unwrap_or(chrono::DateTime::<chrono::Utc>::MAX_UTC)
}

/// Most starts one period of a rule may give (the days it can fall on times the times of a day). A rule
/// that gives more is refused when written: `yearly` with every month, day, hour, minute and second is
/// 31 million starts a year.
pub const MAX_STARTS_PER_PERIOD: u64 = 100_000;

/// Most starts a day a rule without `count` and `until` may give: it overlaps every window from its
/// start on, so its cost is paid by every question.
pub const MAX_UNBOUNDED_STARTS_PER_DAY: u64 = 500;

/// Most steps of a rule the engine may walk from the series start to the end of the window. `rrule`
/// walks from `DTSTART`, so a daily series asked about year 8999 costs seconds. Beyond this the
/// question is refused for that event and reported, instead of freezing every caller.
pub const MAX_RULE_STEPS: u64 = 500_000;
