//! An event as a form.
//!
//! Pattern: Factory — the form always comes from the stored event itself, never from an occurrence, so
//! what one occurrence's override shows can never be saved over the whole series. The times of a series
//! are shown locked.

use chrono::{Duration, NaiveDate};
use fs_calendar_core::Event;
use fs_calendar_core::time::Zone;
use fs_components_workbenches::calendar::{Day, DraftField, FieldKind, ItemDraft};

use super::location::first_name;
use super::when::{
    date_of, date_text, days_covered, duration_of, end_of, start_of, time_text, zone_of,
};
use crate::words::{field, say};

/// The fields of the form.
pub(super) const TITLE: &str = "title";
pub(super) const START: &str = "start";
pub(super) const END: &str = "end";
pub(super) const ALL_DAY: &str = "all_day";
pub(super) const ZONE: &str = "time_zone";
pub(super) const LOCATION: &str = "location";
pub(super) const DESCRIPTION: &str = "description";

/// Does it repeat, or have occurrences of its own?
pub(super) fn is_series(event: &Event) -> bool {
    event.recurrence_rule.is_some() || !event.recurrence_overrides.is_empty()
}

/// Start and end as the form and the sheet show them. For an event without a time the end is the last
/// day it covers, which is how people say it, not the exclusive end the standard counts to.
pub(super) fn when_texts(event: &Event) -> Result<(String, String), String> {
    let start = start_of(event)?;
    if event.show_without_time == Some(true) {
        let first: NaiveDate = start.date();
        let last = first + Duration::days(days_covered(duration_of(event)?) - 1);
        return Ok((date_text(first), date_text(last)));
    }
    // A start in the hour clocks skip is shown as the time it really is, the one the calendar draws.
    let start = zone_of(event).map_or(start, |zone| zone.to_local(zone.to_utc(start)));
    Ok((time_text(start), time_text(end_of(event)?)))
}

/// The form of `event`.
pub(super) fn draft_of(
    event: &Event,
    revision: Option<String>,
    marked: bool,
) -> Result<ItemDraft, String> {
    let (start, end) = when_texts(event)?;
    // Same format, so the order of the texts is the order of the times.
    let end_reads_early = end < start;
    let (place, places) = first_name(event);
    let all_day = event.show_without_time == Some(true);
    let mut time_fields = vec![
        DraftField::new(START, field::START, start),
        DraftField::new(END, field::END, end),
        DraftField::new(ALL_DAY, field::ALL_DAY, all_day.to_string()).kind(FieldKind::Flag),
        DraftField::new(
            ZONE,
            field::ZONE,
            event.time_zone.clone().unwrap_or_default(),
        ),
    ];
    let series = is_series(event);
    if series {
        time_fields = time_fields.into_iter().map(DraftField::locked).collect();
    }
    let mut fields = vec![DraftField::new(
        TITLE,
        field::TITLE,
        event.title.clone().unwrap_or_default(),
    )];
    fields.extend(time_fields);
    fields.push(DraftField::new(LOCATION, field::LOCATION, place));
    fields.push(
        DraftField::new(
            DESCRIPTION,
            field::DESCRIPTION,
            event.description.clone().unwrap_or_default(),
        )
        .kind(FieldKind::LongText),
    );
    let mut draft = ItemDraft::new(revision, fields);
    let mut notes = Vec::new();
    if series {
        notes.push(say::SERIES_FORM);
    }
    if places > 1 {
        notes.push(say::MORE_PLACES);
    }
    if end_reads_early {
        notes.push(say::END_AFTER_FOLD);
    }
    if !notes.is_empty() {
        draft.note = Some(notes.join(" "));
    }
    if marked {
        draft.marks.push(field::SENSITIVE.to_string());
    }
    Ok(draft)
}

/// An empty form for a new event on `day`, at nine for an hour, in the zone shown.
pub(super) fn blank_draft(day: Day, zone: &Zone) -> ItemDraft {
    let date = date_text(date_of(day));
    ItemDraft::new(
        None,
        vec![
            DraftField::new(TITLE, field::TITLE, ""),
            DraftField::new(START, field::START, format!("{date} 09:00")),
            DraftField::new(END, field::END, format!("{date} 10:00")),
            DraftField::new(ALL_DAY, field::ALL_DAY, "false").kind(FieldKind::Flag),
            DraftField::new(ZONE, field::ZONE, zone.name()),
            DraftField::new(LOCATION, field::LOCATION, ""),
            DraftField::new(DESCRIPTION, field::DESCRIPTION, "").kind(FieldKind::LongText),
        ],
    )
}
