//! What is on some days, whatever the core makes of it.
//!
//! Pattern: Strategy by fallback — first the contract's agenda. If that fails as a whole, one damaged
//! event is the likely cause, so each event is expanded on its own and only the damaged one is named.
//! A series with an override moved before its first start is not offered by the store for that time
//! (its envelope starts at the first start), so those are remembered and expanded here as well. Own
//! writes forget what was remembered; a change by another program shows after a restart.

use std::collections::HashSet;
use std::sync::{Mutex, PoisonError};

use chrono::{DateTime, Utc};
use fs_calendar_core::recur::{RRuleEngine, RecurrenceEngine, expand, zone_of as recur_zone_of};
use fs_calendar_core::time::parse_local;
use fs_calendar_core::{Agenda, Error, Event, Instance};
use fs_components_workbenches::SourceError;
use fs_components_workbenches::calendar::{Day, ItemProblem};
use fs_store::{ObjectId, StoredObject};
use fs_tasks_core::TaskEntry;

use super::adapter::{CalendarAdapter, reason};
use super::items::window;
use super::when::{duration_of, start_of};
use crate::words::say;

/// The core reads at most this many occurrences of one rule per question (`rrule`'s limit).
pub(super) const CUT_AT: usize = u16::MAX as usize;

/// Events and tasks in the window, and what could not be read.
#[derive(Default)]
pub(super) struct Found {
    pub events: Vec<Instance>,
    pub tasks: Vec<TaskEntry>,
    pub problems: Vec<ItemProblem>,
}

/// The series with an override moved before their first start, once they have been looked for.
#[derive(Default)]
pub(super) struct EarlyMovers(Mutex<Option<Vec<(ObjectId, Event)>>>);

impl EarlyMovers {
    /// Something was written: look again next time.
    pub(super) fn forget(&self) {
        *self.0.lock().unwrap_or_else(PoisonError::into_inner) = None;
    }
}

/// `{title}: cannot be shown ({why})`.
pub(super) fn damaged(id: ObjectId, title: &str, why: &str) -> ItemProblem {
    ItemProblem::of(
        id.to_string(),
        say::DAMAGED.replace("{title}", title).replace("{why}", why),
    )
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
    /// Events and tasks on the days `first` through `last`, with a margin (see `items::window`).
    pub(super) fn found(&self, first: Day, last: Day) -> Result<Found, SourceError> {
        let (from, to) = window(first, last, &self.zone);
        let agenda = match self.runtime.block_on(self.calendar.agenda(from, to)) {
            Ok(agenda) => Ok(agenda),
            Err(Error::Unsupported(_)) => self
                .runtime
                .block_on(self.calendar.occurrences(from, to))
                .map(|events| Agenda {
                    events,
                    tasks: Vec::new(),
                }),
            Err(e) => Err(e),
        };
        let Ok(agenda) = agenda else {
            return self.event_by_event(from, to);
        };
        let mut found = Found {
            events: agenda.events,
            tasks: agenda.tasks,
            problems: Vec::new(),
        };
        self.add_early_movers(&mut found, from, to)?;
        Ok(found)
    }

    /// Did the limit of the rule engine cut the occurrences of `event` on these days? Asks the engine
    /// the same question the core asks it, with the same margin, since the core does not say.
    pub(super) fn cut_by_core(&self, event: &Event, first: Day, last: Day) -> bool {
        let (Some(rule), Ok(start), Ok(length)) =
            (&event.recurrence_rule, start_of(event), duration_of(event))
        else {
            return false;
        };
        let (Ok(zone), (from, to)) = (
            recur_zone_of(event, &self.zone),
            window(first, last, &self.zone),
        ) else {
            return false;
        };
        let margin = chrono::Duration::seconds(length.max_secs() + 2 * 86_400);
        RRuleEngine
            .starts(rule, start, &zone, from - margin, to)
            .is_ok_and(|starts| starts.len() >= CUT_AT)
    }

    /// One event at a time: what is damaged is named, the rest is there. Tasks come with the agenda,
    /// so they are missing here, and the person is told.
    fn event_by_event(&self, from: DateTime<Utc>, to: DateTime<Utc>) -> Result<Found, SourceError> {
        let all = self
            .runtime
            .block_on(self.calendar.list_events())
            .map_err(|e| reason(&e))?;
        let mut found = Found::default();
        for object in &all {
            self.expand_one(object, from, to, &mut found);
        }
        found.problems.push(ItemProblem::note(say::TASKS_LOST));
        Ok(found)
    }

    fn expand_one(
        &self,
        object: &StoredObject,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
        found: &mut Found,
    ) {
        let (id, title) = (object.envelope.id, &object.envelope.name);
        let expanded = Event::from_value(object.content.clone())
            .map_err(|e| e.to_string())
            .and_then(|event| {
                expand(&event, from, to, &self.zone, usize::MAX).map_err(|e| e.to_string())
            });
        match expanded {
            Ok(occurrences) => found
                .events
                .extend(occurrences.into_iter().map(|occurrence| Instance {
                    object: id,
                    occurrence,
                })),
            Err(why) => found.problems.push(damaged(id, title, &why)),
        }
    }

    fn add_early_movers(
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
