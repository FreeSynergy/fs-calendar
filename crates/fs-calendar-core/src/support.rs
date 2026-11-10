//! The support table: which commands of the `Event` description the core really does.
//!
//! Pattern: Registry (one table, one place) — every command of the description is listed here with a flag and a
//! reason; `describe` reads the command names from it and `EventBackend` asks `is_supported` before it runs one.
//! `types.capnp` has no `supported` field yet (ADR 0066, thread T adds it): when it does, the flag of this table is
//! copied into the description in `describe::commands` and nowhere else. `covers` names the capability of
//! `docs/en/programs/apps/calendar/requirements.md` a command answers, word for word, so a test can show that
//! none is missing.

/// One command of the description.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Support {
    /// The command name in the description.
    pub command: &'static str,
    /// The core does it now.
    pub supported: bool,
    /// Why not (or what does it); an English note for the log, not a translation key.
    pub reason: &'static str,
    /// The capabilities of `requirements.md` (first column, exactly) this command stands for.
    pub covers: &'static [&'static str],
}

const fn row(
    command: &'static str,
    supported: bool,
    reason: &'static str,
    covers: &'static [&'static str],
) -> Support {
    Support {
        command,
        supported,
        reason,
        covers,
    }
}

/// Every command, in the order of the description.
pub const TABLE: &[Support] = &[
    row("create", true, "Calendar::create", &[]),
    row("change", true, "Calendar::update_checked, partial", &[]),
    row("delete", true, "Calendar::delete", &[]),
    row(
        "find",
        true,
        "list_page, occurrences/window for a range",
        &[],
    ),
    row(
        "agenda",
        true,
        "Calendar::agenda: events and the tasks due (through the tasks interface)",
        &[],
    ),
    row("import", true, "Calendar::import_text", &[]),
    row("export", true, "Calendar::export_text", &[]),
    row("invite", true, "Calendar::invite", &[]),
    row("respond", true, "Calendar::respond", &[]),
    row("history", true, "Calendar::invitation_log", &[]),
    row("check", true, "validate::issues over the stored event", &[]),
    row(
        "suggest",
        false,
        "the core has no source of suggestions for event fields",
        &[],
    ),
    row(
        "quickEntry",
        false,
        "no parser for a typed sentence yet",
        &[
            "Create an event or task from one typed sentence (date, time, place, calendar, alert, repetition)",
        ],
    ),
    row(
        "showZones",
        false,
        "no basic type for a time zone, no zone preference in the core",
        &["Show several time zones side by side, labelled; temporary switch of the primary zone"],
    ),
    row(
        "shareFreeSlots",
        false,
        "no free/busy computation and no link service yet",
        &["Share free slots as a link (one-off or recurring) with booking window and expiry"],
    ),
    row(
        "bookingPage",
        false,
        "no availability model and no public page yet",
        &["Public booking page per event type with availability, buffers, minimum notice"],
    ),
    row(
        "planTasks",
        false,
        "no planner; tasks stay with fs-tasks",
        &[
            "Place tasks into free calendar time by priority, deadline and duration, and re-plan on change",
        ],
    ),
    row(
        "flexibleBlock",
        false,
        "no flexible blocks; needs a recurrence basic type and a planner",
        &[
            "Repeating flexible blocks (habits, focus time) with frequency, duration and time window",
        ],
    ),
];

/// Properties of a JSCalendar `Event` that the description does not carry, and why (no basic type for them yet).
pub const NOT_DESCRIBED: &[(&str, &str)] = &[
    ("recurrenceRule", "no basic type for a recurrence rule"),
    ("recurrenceOverrides", "no basic type for patch objects"),
    ("alerts", "no basic type for an alert (trigger, action)"),
    ("relatedTo", "no basic type for a typed relation"),
    ("localizations", "no basic type for a patch per language"),
];

/// The row of a command.
#[must_use]
pub fn find(command: &str) -> Option<&'static Support> {
    TABLE.iter().find(|r| r.command == command)
}

/// Whether the core does this command. An unknown name is not supported.
#[must_use]
pub fn is_supported(command: &str) -> bool {
    find(command).is_some_and(|r| r.supported)
}

/// The names of the commands, in order.
pub fn names() -> impl Iterator<Item = &'static str> {
    TABLE.iter().map(|r| r.command)
}
