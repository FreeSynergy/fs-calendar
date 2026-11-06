//! Both sides changed the times.
//!
//! Pattern: Specification over the form and the event as it is now — what the form started with is what
//! the person saw. Start, end, all-day and zone belong together: one of them typed against others moved
//! elsewhere has no meaning anyone asked for, so such a save is refused and says what changed elsewhere.
//! Other fields merge.

use fs_calendar_core::Event;
use fs_components_workbenches::calendar::ItemDraft;

use super::draft::{ALL_DAY, END, START, ZONE, when_texts};
use crate::words::{field, say};

/// Why a save of `draft` onto `event` must not write its times, if so.
pub(super) fn times_clash(event: &Event, draft: &ItemDraft) -> Result<Option<String>, String> {
    // A new event has nothing to clash with.
    if draft.revision.is_none() {
        return Ok(None);
    }
    let (start, end) = when_texts(event)?;
    let all_day = (event.show_without_time == Some(true)).to_string();
    let zone = event.time_zone.clone().unwrap_or_default();
    let now = [
        (START, field::START, start),
        (END, field::END, end),
        (ALL_DAY, field::ALL_DAY, all_day),
        (ZONE, field::ZONE, zone),
    ];
    let mut changes = Vec::new();
    let mut typed = false;
    for (key, label, value) in now {
        let Some(seen) = draft.fields.iter().find(|f| f.key == key) else {
            continue;
        };
        typed |= seen.changed();
        if seen.original != value {
            changes.push(format!("{label} is now {}", shown(key, &value)));
        }
    }
    Ok((typed && !changes.is_empty())
        .then(|| say::TIMES_CLASH.replace("{changes}", &changes.join(", "))))
}

fn shown<'a>(key: &str, value: &'a str) -> &'a str {
    match (key, value) {
        (ALL_DAY, "true") => field::YES,
        (ALL_DAY, _) => field::NO,
        (_, "") => field::NONE,
        _ => value,
    }
}
