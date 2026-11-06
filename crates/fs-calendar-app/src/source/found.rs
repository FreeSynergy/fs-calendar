//! What is on some days, whatever the core makes of it.
//!
//! Pattern: Strategy by fallback — first the contract's agenda. If that fails as a whole, one damaged
//! event is the likely cause, so each event is expanded on its own and only the damaged one is named.

use chrono::{DateTime, Utc};
use fs_calendar_core::recur::{RRuleEngine, RecurrenceEngine, expand, zone_of as recur_zone_of};
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

/// `{title}: cannot be shown ({why})`.
pub(super) fn damaged(id: ObjectId, title: &str, why: &str) -> ItemProblem {
    ItemProblem::of(
        id.to_string(),
        say::DAMAGED.replace("{title}", title).replace("{why}", why),
    )
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
}
