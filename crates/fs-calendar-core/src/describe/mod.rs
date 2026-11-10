//! The description of an event: what a generated frontend is told (ADR 0066).
//!
//! Pattern: Registry entry (trait first, ADR 0043) — [`Event`] implements `DescribesType` from `fs-types`, so
//! `TypeRegistry::register::<Event>()` puts it in the catalog the program serves. Fields come from the table in
//! `fields.rs`, commands from `commands.rs` and the support table (`crate::support`).

mod commands;
mod fields;

use fs_types::{DescribesType, FieldDescription, TypeDescription};

pub use fields::{FREE_BUSY, PRIORITY, PRIVACY, STATUS, YES_NO};

use crate::model::Event;

/// The name of the type in the catalog.
pub const EVENT: &str = "Event";

/// The program that owns it (ADR 0050).
pub const PROGRAM: &str = "fs-calendar";

/// The standard it follows.
pub const STANDARD: &str = "JSCalendar Event (RFC 8984)";

/// `timeZone` -> `time-zone`: the translation key part of a property name.
#[must_use]
pub fn kebab(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        if c.is_ascii_uppercase() {
            out.push('-');
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// A field of the type or (with `scope`) of a command: `event.<name>` or `event.<command>.<name>`.
pub(crate) fn describe_field(scope: Option<&str>, f: &fields::Field) -> FieldDescription {
    let key = match scope {
        Some(c) => format!("event.{}.{}", kebab(c), kebab(f.name)),
        None => format!("event.{}", kebab(f.name)),
    };
    let mut d = FieldDescription::new(f.name, f.basic).label(&key);
    if f.required {
        d = d.required();
    }
    if f.multiple {
        d = d.multiple();
    }
    if !f.target.is_empty() {
        d = d.target(f.target);
    }
    if !f.choices.is_empty() {
        d = d.choices(f.choices);
    }
    d
}

/// An event is described by its fields, the properties of the JSCalendar `Event`.
///
/// Not described (no basic type for it yet, see `crate::support::NOT_DESCRIBED` and `DESIGN.md`): `recurrenceRule`,
/// `recurrenceOverrides`, `alerts`, `relatedTo`, `localizations`. The commands are all of `requirements.md`; the
/// ones the core does not do carry `supported = false` in `crate::support`.
impl DescribesType for Event {
    fn describe() -> TypeDescription {
        let mut t = TypeDescription::new(EVENT, PROGRAM, STANDARD, "event");
        for f in &fields::table() {
            t = t.field(describe_field(None, f));
        }
        for c in commands::all() {
            t = t.command(c);
        }
        t
    }
}
