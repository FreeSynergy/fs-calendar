//! The commands of the `Event` description.
//!
//! Pattern: Builder over the support table — the names and their order come from `crate::support::TABLE`, the input
//! and result fields from the functions here; a command the table does not know cannot be built (`input_of`), so
//! the table is the one list of commands and a test shows that the description has all of them.

use fs_types::{BasicType, CommandDescription, CommandKind, FieldDescription};

use super::{EVENT, describe_field, fields};
use crate::support;

/// A field of a command: `event.<command>.<name>`.
fn input(command: &str, name: &str, basic: BasicType) -> FieldDescription {
    FieldDescription::new(name, basic).label(&format!(
        "event.{}.{}",
        super::kebab(command),
        super::kebab(name)
    ))
}

fn event_ref(command: &str, name: &str) -> FieldDescription {
    input(command, name, BasicType::Reference).target(EVENT)
}

fn whole(command: &str) -> Vec<FieldDescription> {
    fields::table()
        .iter()
        .map(|f| describe_field(Some(command), f))
        .collect()
}

fn with_inputs(mut c: CommandDescription, list: Vec<FieldDescription>) -> CommandDescription {
    for f in list {
        c = c.input(f);
    }
    c
}

fn make(name: &str, kind: CommandKind) -> CommandDescription {
    CommandDescription::new(name, kind, &format!("event.{}", super::kebab(name)))
}

/// The command `name`; every name of the support table has one.
fn build(name: &str) -> Option<CommandDescription> {
    let text = |n: &str| input(name, n, BasicType::Text);
    let at = |n: &str| input(name, n, BasicType::DateTime);
    let many = |f: FieldDescription| f.multiple();
    Some(match name {
        "create" => with_inputs(make(name, CommandKind::Create), whole(name))
            .result(event_ref(name, "id").required()),
        // Kind `change`: its input is an `Edit` (field path, basic type, values; rule of `types.capnp`), so the
        // list of input fields is empty and the record's id travels next to it.
        "change" => make(name, CommandKind::Change),
        "delete" => make(name, CommandKind::Delete).input(event_ref(name, "id").required()),
        // A list is `find` with an empty `term` (rule of `types.capnp`); with `from` and `to` the answer is the
        // occurrences of the range, a series expanded.
        "find" => CommandDescription::standard_find("event.find", EVENT)
            .input(at("from"))
            .input(at("to")),
        "agenda" => make(name, CommandKind::Other)
            .input(at("from").required())
            .input(at("to").required())
            .result(many(event_ref(name, "events")))
            .result(many(
                input(name, "tasks", BasicType::Reference).target("Task"),
            )),
        "import" => make(name, CommandKind::Other)
            .input(
                input(name, "format", BasicType::Choice)
                    .choices(&["ics", "jscalendar"])
                    .required(),
            )
            .input(input(name, "text", BasicType::MultilineText).required())
            .result(many(event_ref(name, "created")))
            .result(many(text("skipped")))
            .result(many(text("failed")))
            .result(many(text("notes"))),
        "export" => make(name, CommandKind::Other)
            .input(many(event_ref(name, "ids")).required())
            .input(
                input(name, "format", BasicType::Choice)
                    .choices(&["ics", "jscalendar"])
                    .required(),
            )
            .result(input(name, "text", BasicType::MultilineText)),
        "invite" => make(name, CommandKind::Other)
            .input(event_ref(name, "id").required())
            .input(text("name").required())
            .input(input(name, "email", BasicType::Email).required()),
        "respond" => make(name, CommandKind::Other)
            .input(event_ref(name, "id").required())
            .input(text("who").required())
            .input(
                input(name, "response", BasicType::Choice)
                    .choices(&["accept", "tentative-accept", "reject"])
                    .required(),
            )
            .input(text("comment")),
        "history" => make(name, CommandKind::Other)
            .input(event_ref(name, "id").required())
            .result(many(text("entries"))),
        "check" => make(name, CommandKind::Check)
            .input(event_ref(name, "id").required())
            .result(many(text("problems"))),
        "suggest" => make(name, CommandKind::Suggest)
            .input(text("field").required())
            .input(text("typed").required())
            .result(many(text("suggestions"))),
        "quickEntry" => make(name, CommandKind::Create)
            .input(input(name, "sentence", BasicType::Text).required())
            .result(event_ref(name, "id")),
        "showZones" => make(name, CommandKind::Other)
            .input(many(text("zones")).required())
            .input(text("primary")),
        "shareFreeSlots" => make(name, CommandKind::Other)
            .input(at("from").required())
            .input(at("to").required())
            .input(at("expires"))
            .input(input(name, "repeat", BasicType::Choice).choices(&["once", "recurring"]))
            .result(input(name, "link", BasicType::Link)),
        "bookingPage" => make(name, CommandKind::Create)
            .input(text("title").required())
            .input(text("duration").required())
            .input(text("buffer"))
            .input(text("minimumNotice"))
            .result(input(name, "link", BasicType::Link)),
        "planTasks" => make(name, CommandKind::Other)
            .input(at("from").required())
            .input(at("to").required())
            .result(many(event_ref(name, "events"))),
        "flexibleBlock" => make(name, CommandKind::Create)
            .input(text("title").required())
            .input(text("frequency").required())
            .input(text("duration").required())
            .input(text("windowStart"))
            .input(text("windowEnd"))
            .result(event_ref(name, "id")),
        _ => return None,
    })
}

/// All commands, in the order of the support table.
#[must_use]
pub fn all() -> Vec<CommandDescription> {
    support::TABLE
        .iter()
        .filter_map(|row| {
            let command = build(row.command)?;
            // SWITCH: the one place where the local flag of `support::TABLE` becomes the `supported` field of the
            // schema (`CommandDescription::unsupported()` = "kommt noch"). When the core reports its commands by
            // itself, only this line changes.
            Some(if row.supported {
                command
            } else {
                command.unsupported()
            })
        })
        .collect()
}
