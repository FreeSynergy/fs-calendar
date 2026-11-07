//! Series with an occurrence before their first start.
//!
//! Pattern: Decorator of the agenda's answer — the store does not offer such a series for that time,
//! because its envelope starts at the first start, so at each reading those series are found in the
//! list of events and expanded here as well. Nothing is remembered, so what changed or went elsewhere
//! is right at once; the numbers of what that costs are in `DESIGN.md`. Provisional until the core's
//! envelope starts at the earliest occurrence.

use std::collections::HashSet;

use chrono::{DateTime, Utc};
use fs_calendar_core::recur::expand;
use fs_calendar_core::time::parse_local;
use fs_calendar_core::{Event, Instance};
use fs_components_workbenches::SourceError;
use fs_objects::{ObjectId, StoredObject};

use super::adapter::{CalendarAdapter, reason};
use super::found::{Found, caught, damaged};
use super::marks::is_marked;

/// Does an occurrence of this series lie before its first start: an override moved there, or an extra
/// occurrence given there?
fn has_early(event: &Event) -> bool {
    let Some(first) = event.start.as_deref().and_then(|s| parse_local(s).ok()) else {
        return false;
    };
    let early = |text: Option<&str>| {
        text.and_then(|s| parse_local(s).ok())
            .is_some_and(|time| time < first)
    };
    event.recurrence_overrides.iter().any(|(key, patch)| {
        patch["excluded"] != serde_json::Value::Bool(true)
            && (early(patch["start"].as_str())
                || (patch.get("start").is_none() && early(Some(key))))
    })
}

impl CalendarAdapter {
    pub(super) fn add_early(
        &self,
        found: &mut Found,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<(), SourceError> {
        let all = self
            .runtime
            .block_on(self.calendar.list_events())
            .map_err(|e| reason(&e))?;
        let known: HashSet<(ObjectId, String)> = found
            .events
            .iter()
            .map(|i| (i.object, i.occurrence.recurrence_id.clone()))
            .collect();
        let series = all
            .iter()
            .filter(|object| object.content.get("recurrenceOverrides").is_some());
        for object in series {
            let Ok(event) = Event::from_value(object.content.clone()) else {
                continue;
            };
            if has_early(&event) {
                self.add_series(found, object, &event, &known, (from, to));
            }
        }
        Ok(())
    }

    fn add_series(
        &self,
        found: &mut Found,
        object: &StoredObject,
        event: &Event,
        known: &HashSet<(ObjectId, String)>,
        (from, to): (DateTime<Utc>, DateTime<Utc>),
    ) {
        let id = object.envelope.id;
        let expanded = caught(|| expand(event, from, to, &self.zone, usize::MAX))
            .and_then(|read| read.map_err(|e| e.to_string()));
        match expanded {
            Ok(occurrences) => found.events.extend(
                occurrences
                    .into_iter()
                    .filter(|o| !known.contains(&(id, o.recurrence_id.clone())))
                    .map(|occurrence| Instance {
                        object: id,
                        occurrence,
                    }),
            ),
            Err(why) => {
                let title = event.display_title();
                found
                    .problems
                    .push(damaged(id, &title, &why, is_marked(object)));
            }
        }
    }
}
