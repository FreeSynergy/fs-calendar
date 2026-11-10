//! The fields of an `Event` as the type description tells them.
//!
//! Pattern: Registry (one table) — one row per JSCalendar property the description carries; `describe` turns the rows
//! into field descriptions, the create and change commands reuse them as input. Kinds the basic types do not have
//! yet are `Text` with the rule named in `DESIGN.md` (time zone, duration, local date-time, boolean, number).

use fs_types::BasicType;

/// One property.
pub struct Field {
    /// JSCalendar property name (lower camel case).
    pub name: &'static str,
    /// Basic type of each value.
    pub basic: BasicType,
    /// Several values.
    pub multiple: bool,
    /// Needed on `create`.
    pub required: bool,
    /// For a reference: the target type.
    pub target: &'static str,
    /// For a choice: the keys.
    pub choices: &'static [&'static str],
}

const fn f(name: &'static str, basic: BasicType) -> Field {
    Field {
        name,
        basic,
        multiple: false,
        required: false,
        target: "",
        choices: &[],
    }
}

/// `true` / `false` (no boolean basic type yet).
pub const YES_NO: &[&str] = &["true", "false"];
/// JSCalendar `status` of an Event.
pub const STATUS: &[&str] = &["confirmed", "tentative", "cancelled"];
/// JSCalendar `freeBusyStatus`.
pub const FREE_BUSY: &[&str] = &["free", "busy"];
/// JSCalendar `privacy`.
pub const PRIVACY: &[&str] = &["public", "private", "secret"];
/// JSCalendar `priority` (0 = undefined, 1 = highest).
pub const PRIORITY: &[&str] = &["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"];

/// The table, in the order of the form.
#[must_use]
pub fn table() -> Vec<Field> {
    let choice = |name, keys| Field {
        choices: keys,
        ..f(name, BasicType::Choice)
    };
    vec![
        f("title", BasicType::Text),
        f("description", BasicType::MultilineText),
        Field {
            required: true,
            ..f("start", BasicType::Text)
        },
        f("duration", BasicType::Text),
        f("timeZone", BasicType::Text),
        f("endTimeZone", BasicType::Text),
        choice("showWithoutTime", YES_NO),
        choice("status", STATUS),
        choice("freeBusyStatus", FREE_BUSY),
        choice("privacy", PRIVACY),
        choice("priority", PRIORITY),
        f("keywords", BasicType::Keywords),
        Field {
            multiple: true,
            ..f("locations", BasicType::Place)
        },
        Field {
            multiple: true,
            target: "Person",
            ..f("participants", BasicType::Reference)
        },
        Field {
            multiple: true,
            ..f("links", BasicType::Link)
        },
        f("color", BasicType::Text),
    ]
}
