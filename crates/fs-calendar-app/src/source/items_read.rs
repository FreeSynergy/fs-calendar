//! The items of some days, and what could not be shown.
//!
//! Pattern: Template method — instances become items one by one; what fails is named once per event and
//! the rest stays. Each stored event is read once, however many occurrences it has.

use std::collections::HashMap;
use std::collections::hash_map::Entry;

use fs_calendar_core::Event;
use fs_components_workbenches::SourceError;
use fs_components_workbenches::calendar::{Day, ItemProblem, ItemsRead};
use fs_store::{ObjectId, StoredObject};

use super::adapter::{CalendarAdapter, event_of};
use super::draft::is_series;
use super::found::{CUT_AT, damaged};
use super::items::{is_within, item_of_instance, item_of_task};
use super::marks::is_marked;
use crate::words::{field, say};

/// With this many occurrences on the days it is worth asking whether the core cut some.
const LOOK_CLOSER_AT: usize = 10_000;

/// What the items need to know of a stored event, read once however many occurrences it has.
struct Master {
    title: String,
    event: Event,
    series: bool,
    marked: bool,
    count: usize,
    named: bool,
}

impl Master {
    fn of(object: &StoredObject) -> Result<Self, SourceError> {
        let event = event_of(object)?;
        Ok(Self {
            title: object.envelope.name.clone(),
            series: is_series(&event),
            event,
            marked: is_marked(object),
            count: 0,
            named: false,
        })
    }
}

impl CalendarAdapter {
    pub(super) fn items_read(&self, first: Day, last: Day) -> Result<ItemsRead, SourceError> {
        let found = self.found(first, last)?;
        let mut problems = found.problems;
        let mut masters: HashMap<ObjectId, Master> = HashMap::new();
        let mut items = Vec::new();
        for instance in &found.events {
            let id = instance.object;
            let master = match masters.entry(id) {
                Entry::Occupied(known) => known.into_mut(),
                Entry::Vacant(new) => match self.stored(id).and_then(|o| Master::of(&o)) {
                    Ok(master) => new.insert(master),
                    Err(e) => {
                        problems.push(damaged(id, &id.to_string(), &e.message, false));
                        continue;
                    }
                },
            };
            master.count += 1;
            let key = if master.series {
                format!("{id}#{}", instance.occurrence.recurrence_id)
            } else {
                id.to_string()
            };
            match item_of_instance(instance, key, &self.zone, master.marked) {
                Ok(item) => {
                    if is_within(&item, first, last) {
                        items.push(item);
                    }
                }
                Err(why) => {
                    if !master.named {
                        master.named = true;
                        problems.push(damaged(id, &master.title, &why, master.marked));
                    }
                }
            }
        }
        for (id, master) in &masters {
            if master.count >= LOOK_CLOSER_AT && self.cut_by_core(&master.event, first, last) {
                let called = if master.marked {
                    say::MARKED_ONE
                } else {
                    &master.title
                };
                let text = say::TOO_MANY
                    .replace("{title}", called)
                    .replace("{count}", &CUT_AT.to_string());
                let problem = ItemProblem::of(id.to_string(), text);
                problems.push(if master.marked {
                    problem.marked(vec![field::SENSITIVE.to_string()])
                } else {
                    problem
                });
            }
        }
        for entry in &found.tasks {
            if let Some(item) =
                item_of_task(entry, &self.zone).filter(|i| is_within(i, first, last))
            {
                items.push(item);
            }
        }
        Ok(ItemsRead::with_problems(items, problems))
    }
}
