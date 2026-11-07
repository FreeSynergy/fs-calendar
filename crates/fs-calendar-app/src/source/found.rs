//! What is on some days, whatever the core makes of it.
//!
//! Pattern: Strategy by fallback — first the contract's agenda. If that fails as a whole, one damaged
//! event is the likely cause, so each event is expanded on its own and only the damaged one is named,
//! and the tasks are read on their own. If the task source is what failed, that is what is said.

use std::panic::{AssertUnwindSafe, catch_unwind};

use chrono::{DateTime, Utc};
use fs_calendar_client::TaskLine;
use fs_calendar_core::recur::{RRuleEngine, RecurrenceEngine, expand, zone_of as recur_zone_of};
use fs_calendar_core::{Event, Instance};
use fs_components_workbenches::SourceError;
use fs_components_workbenches::calendar::{Day, ItemProblem};
use fs_store::{ObjectId, StoredObject};

use super::adapter::{CalendarAdapter, reason};
use super::items::window;
use super::marks::is_marked;
use super::when::{duration_of, start_of};
use crate::words::{field, say};

/// The core reads at most this many occurrences of one rule per question (`rrule`'s limit).
pub(super) const CUT_AT: usize = u16::MAX as usize;

/// Events and tasks in the window, and what could not be read.
#[derive(Default)]
pub(super) struct Found {
    pub events: Vec<Instance>,
    pub tasks: Vec<TaskLine>,
    /// Why tasks could not be read, if they could not.
    pub tasks_unread: Option<String>,
    pub problems: Vec<ItemProblem>,
}

/// `f`, with a panic of the time library turned into a reason: the window must start and name what is
/// damaged. Rust still prints the panic on stderr.
pub(crate) fn caught<T>(f: impl FnOnce() -> T) -> Result<T, String> {
    catch_unwind(AssertUnwindSafe(f)).map_err(|_| say::CAUGHT.to_string())
}

/// `{title}: cannot be shown ({why})`; something marked is not called by its title, and carries its mark.
pub(super) fn damaged(id: ObjectId, title: &str, why: &str, marked: bool) -> ItemProblem {
    let called = if marked { say::MARKED_ONE } else { title };
    let problem = ItemProblem::of(
        id.to_string(),
        say::DAMAGED
            .replace("{title}", called)
            .replace("{why}", why),
    );
    if marked {
        problem.marked(vec![field::SENSITIVE.to_string()])
    } else {
        problem
    }
}

impl CalendarAdapter {
    /// Events and tasks on the days `first` through `last`, with a margin (see `items::window`).
    pub(super) fn found(&self, first: Day, last: Day) -> Result<Found, SourceError> {
        let (from, to) = window(first, last, &self.zone);
        let mut found = match caught(|| self.runtime.block_on(self.calendar.window(from, to))) {
            Ok(Ok(window)) if window.damaged.is_empty() => {
                let mut found = Found {
                    events: window.instances,
                    ..Found::default()
                };
                self.verify(&mut found, from, to)?;
                found
            }
            // The server could not answer for the window as a whole (a damaged event, a cut): one
            // event at a time, so only the damaged one is named.
            _ => self.event_by_event(from, to)?,
        };
        self.read_tasks(&mut found, from, to);
        Ok(found)
    }

    /// The tasks due in the window, from the tasks program if one is connected; if it cannot be read
    /// that is said above the calendar and the events are shown all the same.
    fn read_tasks(&self, found: &mut Found, from: DateTime<Utc>, to: DateTime<Utc>) {
        let Some(tasks) = &self.tasks else { return };
        match caught(|| self.runtime.block_on(tasks.due_between(from, to))) {
            Ok(Ok(lines)) => found.tasks = lines,
            other => {
                let why = match other {
                    Ok(Err(e)) => e.to_string(),
                    _ => say::CAUGHT.to_string(),
                };
                let why = say::TASKS_FAILED.replace("{why}", &why);
                found.problems.push(ItemProblem::note(why.clone()));
                found.tasks_unread = Some(why);
            }
        }
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

    /// One event at a time: what is damaged is named, the rest is there.
    fn event_by_event(&self, from: DateTime<Utc>, to: DateTime<Utc>) -> Result<Found, SourceError> {
        let all = self
            .runtime
            .block_on(self.calendar.list_events())
            .map_err(|e| reason(&e))?;
        let mut found = Found::default();
        for object in &all {
            self.expand_one(object, from, to, &mut found);
        }
        Ok(found)
    }

    /// The instances of one stored event in the window, expanded here with the same engine.
    pub(super) fn expand_local(
        &self,
        object: &StoredObject,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<Instance>, String> {
        let event = Event::from_value(object.content.clone()).map_err(|e| e.to_string())?;
        let occurrences = caught(|| expand(&event, from, to, &self.zone, usize::MAX))?
            .map_err(|e| e.to_string())?;
        let id = object.envelope.id;
        Ok(occurrences
            .into_iter()
            .map(|occurrence| Instance {
                object: id,
                occurrence,
            })
            .collect())
    }

    pub(super) fn expand_one(
        &self,
        object: &StoredObject,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
        found: &mut Found,
    ) {
        match self.expand_local(object, from, to) {
            Ok(instances) => found.events.extend(instances),
            Err(why) => found.problems.push(damaged(
                object.envelope.id,
                &object.envelope.name,
                &why,
                is_marked(object),
            )),
        }
    }
}
