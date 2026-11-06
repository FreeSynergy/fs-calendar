//! Series with an override moved before their first start.
//!
//! Pattern: Memento — the store does not offer such a series for that time, because its envelope starts
//! at the first start, so those series are looked for once, remembered and expanded here as well. Own
//! writes forget what was remembered; a change by another program shows after a restart. Provisional
//! until the core's envelope starts at the earliest occurrence.

use std::collections::HashSet;
use std::sync::{Mutex, PoisonError};

use chrono::{DateTime, Utc};
use fs_calendar_core::recur::expand;
use fs_calendar_core::time::parse_local;
use fs_calendar_core::{Event, Instance};
use fs_components_workbenches::SourceError;
use fs_store::ObjectId;

use super::adapter::{CalendarAdapter, reason};
use super::found::{Found, damaged};

/// The series with an override moved before their first start, once they have been looked for.
#[derive(Default)]
pub(super) struct EarlyMovers(Mutex<Option<Vec<(ObjectId, Event)>>>);

impl EarlyMovers {
    /// Something was written: look again next time.
    pub(super) fn forget(&self) {
        *self.0.lock().unwrap_or_else(PoisonError::into_inner) = None;
    }
}

/// Does an override of this series start before the series?
fn moves_early(event: &Event) -> bool {
    let Some(first) = event.start.as_deref().and_then(|s| parse_local(s).ok()) else {
        return false;
    };
    event.recurrence_overrides.values().any(|patch| {
        patch["start"]
            .as_str()
            .and_then(|s| parse_local(s).ok())
            .is_some_and(|start| start < first)
    })
}

impl CalendarAdapter {
    pub(super) fn add_early_movers(
        &self,
        found: &mut Found,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<(), SourceError> {
        let held = self
            .early
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        let movers = if let Some(movers) = held {
            movers
        } else {
            let all = self
                .runtime
                .block_on(self.calendar.list_events())
                .map_err(|e| reason(&e))?;
            let movers: Vec<(ObjectId, Event)> = all
                .iter()
                .filter_map(|object| {
                    let event = Event::from_value(object.content.clone()).ok()?;
                    moves_early(&event).then_some((object.envelope.id, event))
                })
                .collect();
            *self.early.0.lock().unwrap_or_else(PoisonError::into_inner) = Some(movers.clone());
            movers
        };
        let known: HashSet<(ObjectId, String)> = found
            .events
            .iter()
            .map(|i| (i.object, i.occurrence.recurrence_id.clone()))
            .collect();
        for (id, event) in movers {
            match expand(&event, from, to, &self.zone, usize::MAX) {
                Ok(occurrences) => found.events.extend(
                    occurrences
                        .into_iter()
                        .filter(|o| !known.contains(&(id, o.recurrence_id.clone())))
                        .map(|occurrence| Instance {
                            object: id,
                            occurrence,
                        }),
                ),
                Err(e) => found
                    .problems
                    .push(damaged(id, &event.display_title(), &e.to_string())),
            }
        }
        Ok(())
    }
}
