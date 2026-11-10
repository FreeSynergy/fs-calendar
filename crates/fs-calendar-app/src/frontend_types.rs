//! TEMPORARY: the type description of the core (fs-types v0.0.11) as the shipped admin frontend takes it (v0.0.2).
//!
//! Pattern: Adapter (anti-corruption layer) — Cargo cannot unify two tags of one repository, and `fs-admin-frontend`
//! (rev a528ea2) is built on fs-types v0.0.2. This file is the only place that knows both. It drops `supported` (the
//! old description has no such field), gives the `change` command the fields the old frontend needs for its form
//! (v0.0.11 has none: the input of a change is an `Edit`) and turns findings back. Remove it when the frontend is
//! tagged on a newer fs-types.

use fs_types as new;
use fs_types_frontend as old;

fn basic(b: new::BasicType) -> old::BasicType {
    use new::BasicType as N;
    use old::BasicType as O;
    match b {
        N::Text => O::Text,
        N::MultilineText => O::MultilineText,
        N::Email => O::Email,
        N::Phone => O::Phone,
        N::Name => O::Name,
        N::Address => O::Address,
        N::Place => O::Place,
        N::Date => O::Date,
        N::DateTime => O::DateTime,
        N::Language => O::Language,
        N::Link => O::Link,
        N::Image => O::Image,
        N::Key => O::Key,
        N::Choice => O::Choice,
        N::Keywords => O::Keywords,
        N::Reference => O::Reference,
        N::Unknown => O::Unknown,
    }
}

fn kind(k: new::CommandKind) -> old::CommandKind {
    use new::CommandKind as N;
    use old::CommandKind as O;
    match k {
        N::Create => O::Create,
        N::Change => O::Change,
        N::Find => O::Find,
        N::Delete => O::Delete,
        N::Check => O::Check,
        N::Suggest => O::Suggest,
        N::Other => O::Other,
        N::Unknown => O::Unknown,
    }
}

fn field(f: &new::FieldDescription) -> old::FieldDescription {
    let mut d = old::FieldDescription::new(&f.name, basic(f.basic)).label(&f.label);
    d.required = f.required;
    d.multiple = f.multiple;
    d.target.clone_from(&f.target);
    d.choices.clone_from(&f.choices);
    d
}

fn command(t: &new::TypeDescription, c: &new::CommandDescription) -> old::CommandDescription {
    let mut out = old::CommandDescription::new(&c.name, kind(c.kind), &c.label);
    if c.kind == new::CommandKind::Change && c.input.is_empty() {
        // The old frontend builds the form of a change from the input and finds the record by a reference to the type.
        let prefix = format!("{}.change", t.label);
        out = out.input(
            old::FieldDescription::new("id", old::BasicType::Reference)
                .label(&format!("{prefix}.id"))
                .target(&t.name)
                .required(),
        );
        for f in &t.fields {
            let mut d = field(f);
            d.required = false;
            d.label = format!("{prefix}.{}", fs_calendar_core::describe::kebab(&f.name));
            out = out.input(d);
        }
    }
    for f in &c.input {
        out = out.input(field(f));
    }
    for f in &c.result {
        out = out.result(field(f));
    }
    out
}

/// The description for the old frontend.
#[must_use]
pub fn description(t: &new::TypeDescription) -> old::TypeDescription {
    let mut out = old::TypeDescription::new(&t.name, &t.program, &t.standard, &t.label);
    for f in &t.fields {
        out = out.field(field(f));
    }
    for c in &t.commands {
        out = out.command(command(t, c));
    }
    out
}

fn type_problem(p: new::TypeProblem) -> old::TypeProblem {
    use new::TypeProblem as N;
    use old::TypeProblem as O;
    match p {
        N::Empty => O::Empty,
        N::TooLong => O::TooLong,
        N::Character | N::Contact(_) => O::Character,
        N::BadLanguage => O::BadLanguage,
        N::BadUri => O::BadUri,
        N::UnsafeScheme => O::UnsafeScheme,
        N::NotAbsolute => O::NotAbsolute,
        N::BadDate => O::BadDate,
        N::BadDateTime => O::BadDateTime,
        N::BadPlace => O::BadPlace,
        N::PlaceRange => O::PlaceRange,
        N::BadFingerprint => O::BadFingerprint,
        N::NotInList => O::NotInList,
        N::NotImage => O::NotImage,
        N::BadKeyword => O::BadKeyword,
        N::NoParts => O::NoParts,
        N::BadCountryCode => O::BadCountryCode,
    }
}

/// The finding for the old frontend; the problems only an edit has are shown as an unknown field.
#[must_use]
pub fn issue(i: &new::RecordIssue) -> old::RecordIssue {
    use new::RecordProblem as N;
    use old::RecordProblem as O;
    let problem = match i.problem {
        N::Missing => O::Missing,
        N::NotMultiple => O::NotMultiple,
        N::Value(p) => O::Value(type_problem(p)),
        N::UnknownField | N::WrongType | N::NestedPath | N::DuplicatePath => O::UnknownField,
    };
    old::RecordIssue {
        field: i.field.clone(),
        index: i.index,
        problem,
    }
}

/// The finding for an input field the description does not have.
#[must_use]
pub fn unknown_field(name: &str) -> old::RecordIssue {
    old::RecordIssue {
        field: name.to_owned(),
        index: 0,
        problem: old::RecordProblem::UnknownField,
    }
}
