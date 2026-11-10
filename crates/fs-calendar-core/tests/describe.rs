//! The description of an `Event`: sound, complete against `requirements.md`, and honest about what the core does.
//!
//! Pattern: none (tests).

use fs_calendar_core::Event;
use fs_calendar_core::describe::EVENT;
use fs_calendar_core::support::{self, NOT_DESCRIBED, TABLE};
use fs_types::{BasicRules, CommandKind, DescribesType, TypeRegistry};

/// `docs/en/programs/apps/calendar/requirements.md` of fs-documentation (origin/main, dac2b9c, 2026-11-10).
const REQUIREMENTS: &str = include_str!("fixtures/requirements.md");

/// The first column of every table row of the Required and Optional sections.
fn capabilities() -> Vec<String> {
    let mut in_table = false;
    let mut out = Vec::new();
    for line in REQUIREMENTS.lines() {
        if line.starts_with("## ") {
            in_table = line.starts_with("## Required") || line.starts_with("## Optional");
            continue;
        }
        if !in_table || !line.starts_with('|') {
            continue;
        }
        let first = line
            .trim_matches('|')
            .split('|')
            .next()
            .unwrap_or("")
            .trim();
        if first.is_empty() || first == "Capability" || first.starts_with("---") {
            continue;
        }
        out.push(first.to_owned());
    }
    out
}

#[test]
fn the_registry_accepts_the_description() {
    let mut registry = TypeRegistry::new();
    registry.register::<Event>().expect("sound description");
    let t = registry.get(EVENT).expect("registered");
    assert_eq!(t.name, "Event");
    assert_eq!(t.program, "fs-calendar");
    assert_eq!(t.standard, "JSCalendar Event (RFC 8984)");
}

#[test]
fn labels_are_translation_keys_not_sentences() {
    let t = Event::describe();
    let all = t.fields.iter().chain(
        t.commands
            .iter()
            .flat_map(|c| c.input.iter().chain(&c.result)),
    );
    for f in all {
        assert!(
            f.label.starts_with("event.") || f.label.starts_with("find."),
            "{}",
            f.label
        );
        assert!(!f.label.contains(' '), "{}", f.label);
    }
    for c in &t.commands {
        assert_eq!(c.label.matches('.').count(), 1, "{}", c.label);
    }
}

#[test]
fn every_capability_of_requirements_has_a_command() {
    let wanted = capabilities();
    assert!(wanted.len() >= 6, "the fixture lost its rows: {wanted:?}");
    let t = Event::describe();
    for w in &wanted {
        let row = TABLE
            .iter()
            .find(|r| r.covers.contains(&w.as_str()))
            .unwrap_or_else(|| panic!("no command for: {w}"));
        assert!(
            t.find_command(row.command).is_some(),
            "{} missing",
            row.command
        );
    }
    // and no command claims a capability that is not in the file
    for r in TABLE {
        for c in r.covers {
            assert!(
                wanted.iter().any(|w| w == c),
                "{} covers an unknown row: {c}",
                r.command
            );
        }
    }
}

#[test]
fn the_description_has_exactly_the_commands_of_the_table_and_they_are_marked() {
    let t = Event::describe();
    let names: Vec<&str> = t.commands.iter().map(|c| c.name.as_str()).collect();
    let table: Vec<&str> = support::names().collect();
    assert_eq!(names, table);
    // what the core does
    for n in [
        "create", "change", "delete", "find", "agenda", "import", "export", "invite", "respond",
        "history", "check",
    ] {
        assert!(support::is_supported(n), "{n}");
    }
    // what it does not (requirements.md and suggest)
    for n in [
        "suggest",
        "quickEntry",
        "showZones",
        "shareFreeSlots",
        "bookingPage",
        "planTasks",
        "flexibleBlock",
    ] {
        assert!(!support::is_supported(n), "{n}");
        assert!(!support::find(n).unwrap().reason.is_empty());
    }
    assert!(!support::is_supported("nothing"));
}

#[test]
fn change_is_an_edit_and_create_has_the_fields() {
    let t = Event::describe();
    let change = t.find_command("change").unwrap();
    assert_eq!(change.kind, CommandKind::Change);
    // the rule of the schema: the input of a change is an `Edit`, the list of input fields is empty
    assert!(change.input.is_empty());
    let create = t.find_command("create").unwrap();
    assert!(create.input.iter().any(|f| f.name == "start" && f.required));
    assert_eq!(create.input.len(), t.fields.len());
    assert_eq!(t.find_command("find").unwrap().kind, CommandKind::Find);
    // the agenda is a command of its own (events and tasks), not a list: a frontend must not route it to `find`
    assert_eq!(t.find_command("agenda").unwrap().kind, CommandKind::Other);
}

#[test]
fn what_the_fields_do_not_carry_is_named() {
    let t = Event::describe();
    for (name, reason) in NOT_DESCRIBED {
        assert!(
            t.find_field(name).is_none(),
            "{name} is described after all"
        );
        assert!(!reason.is_empty());
    }
}

#[test]
fn a_record_is_checked_against_the_description() {
    let t = Event::describe();
    let rules = BasicRules::standard();
    let record = [("status".to_owned(), vec!["maybe".to_owned()])].into();
    let issues = t.check_record(&rules, &record);
    assert!(issues.iter().any(|i| i.field == "status"), "{issues:?}");
    assert!(
        issues.iter().any(|i| i.field == "start"),
        "start is required: {issues:?}"
    );
}

#[test]
fn the_schema_flag_follows_the_table() {
    let t = Event::describe();
    for r in TABLE {
        assert_eq!(
            t.find_command(r.command).unwrap().supported,
            r.supported,
            "{}",
            r.command
        );
    }
    // the list is the standard find: `term` and a list of events
    let find = t.find_command("find").unwrap();
    assert_eq!(find.input[0].name, "term");
    assert_eq!(find.result[0].target, "Event");
    // the commands that work on a record are found by the rule of the schema
    let on_record: Vec<&str> = t
        .record_commands()
        .iter()
        .map(|c| c.name.as_str())
        .collect();
    for n in ["change", "delete", "invite", "respond", "history", "check"] {
        assert!(on_record.contains(&n), "{n}");
    }
}
