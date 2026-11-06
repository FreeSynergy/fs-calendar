//! An event to be read.
//!
//! Pattern: Adapter + Template — fixed sections: when, what, repeats, people, and what else the event
//! holds. Nothing is hidden: properties the sheet has no line for are named.

use fs_calendar_core::recur::rrule_string;
use fs_calendar_core::time::Zone;
use fs_calendar_core::{Event, Instance};
use fs_components_workbenches::calendar::{ItemSheet, Moment, SheetField, SheetSection};
use fs_store::StoredObject;

use super::draft::{is_series, when_texts};
use super::items::moment_in;
use super::location::first_name;
use super::marks::is_marked;
use super::when::{day_of, end_of, start_of};
use crate::words::{field, say};

/// What the event is shown as: the occurrence chosen, if there is one.
pub(super) fn sheet_of(
    object: &StoredObject,
    master: &Event,
    instance: Option<&Instance>,
    zone: &Zone,
) -> Result<ItemSheet, String> {
    let shown = instance.map_or(master, |i| &i.occurrence.event);
    let (start, end) = when_texts(shown)?;
    let mut when = vec![
        SheetField::new(field::START, start),
        SheetField::new(field::END, end),
    ];
    when.push(SheetField::new(
        field::ZONE,
        shown
            .time_zone
            .clone()
            .unwrap_or_else(|| field::FLOATING.to_string()),
    ));
    if shown.show_without_time == Some(true) {
        when.push(SheetField::new(field::ALL_DAY, field::YES));
    }
    if let Some((a, b)) =
        span_in(shown, zone)?.filter(|_| shown.time_zone.as_deref() != Some(zone.name()))
    {
        when.push(SheetField::new(field::HERE, span_text(a, b)));
    }
    let mut sections = vec![SheetSection::new(field::WHEN, when)];
    sections.extend(details(shown));
    let series = is_series(master);
    if series {
        sections.push(repeats(master, instance, zone));
    }
    sections.extend(people(shown));
    sections.extend(others(&object.content));
    let day = match span_in(shown, zone)? {
        Some((from, _)) => from.day,
        None => day_of(start_of(shown)?.date()),
    };
    let mut sheet = ItemSheet::new(shown.display_title(), day, sections);
    if is_marked(object) {
        sheet.marks.push(field::SENSITIVE.to_string());
    }
    if series {
        sheet.note = Some(say::SERIES_SHEET.to_string());
        sheet.delete_warning = Some(say::SERIES_DELETE.to_string());
    }
    Ok(sheet)
}

/// Start and end on the clock of `zone`; `None` for what is not converted: all-day and floating.
fn span_in(event: &Event, zone: &Zone) -> Result<Option<(Moment, Moment)>, String> {
    let own = match event.time_zone.as_deref() {
        Some(name) if event.show_without_time != Some(true) => {
            Zone::parse(name).map_err(|e| e.to_string())?
        }
        _ => return Ok(None),
    };
    let at = |time| moment_in(own.to_utc(time), zone);
    Ok(Some((at(start_of(event)?), at(end_of(event)?))))
}

fn span_text(a: Moment, b: Moment) -> String {
    if a.day == b.day {
        format!("{} {} \u{2013} {}", a.day, a.clock(), b.clock())
    } else {
        format!("{} {} \u{2013} {} {}", a.day, a.clock(), b.day, b.clock())
    }
}

fn details(event: &Event) -> Option<SheetSection> {
    let mut fields = Vec::new();
    let (place, _) = first_name(event);
    if !place.is_empty() {
        fields.push(SheetField::new(field::LOCATION, place));
    }
    if let Some(text) = event.description.as_deref().filter(|t| !t.is_empty()) {
        fields.push(SheetField::new(field::DESCRIPTION, text));
    }
    if let Some(status) = &event.status {
        fields.push(SheetField::new(field::STATUS, status.clone()));
    }
    (!fields.is_empty()).then(|| SheetSection::new(field::WHAT, fields))
}

fn repeats(master: &Event, instance: Option<&Instance>, zone: &Zone) -> SheetSection {
    let mut fields = Vec::new();
    if let Some(rule) = &master.recurrence_rule {
        let text = rrule_string(rule, zone).unwrap_or_default();
        fields.push(SheetField::new(field::RULE, text));
    }
    if let Some(instance) = instance {
        fields.push(SheetField::new(
            field::THIS_ONE,
            instance.occurrence.recurrence_id.clone(),
        ));
    }
    SheetSection::new(field::REPEATS, fields)
}

fn people(event: &Event) -> Option<SheetSection> {
    let fields: Vec<SheetField> = event
        .participants
        .values()
        .map(|p| {
            SheetField::new(
                p.name.clone().unwrap_or_default(),
                p.email
                    .clone()
                    .or_else(|| p.calendar_address.clone())
                    .unwrap_or_default(),
            )
        })
        .collect();
    (!fields.is_empty()).then(|| SheetSection::new(field::PEOPLE, fields))
}

/// The properties the sheet has no line for; they are kept and named, so nothing looks gone.
fn others(content: &serde_json::Value) -> Option<SheetSection> {
    const SHOWN: [&str; 21] = [
        "@type",
        "version",
        "uid",
        "title",
        "start",
        "duration",
        "timeZone",
        "showWithoutTime",
        "description",
        "locations",
        "status",
        "participants",
        "recurrenceRule",
        "recurrenceOverrides",
        "created",
        "updated",
        "sequence",
        "prodId",
        "recurrenceId",
        "descriptionContentType",
        "privacy",
    ];
    let names: Vec<&str> = content
        .as_object()?
        .keys()
        .map(String::as_str)
        .filter(|key| !SHOWN.contains(key))
        .collect();
    (!names.is_empty()).then(|| {
        SheetSection::new(
            field::OTHER,
            vec![SheetField::new(field::OTHER_LABEL, names.join(", "))],
        )
    })
}
