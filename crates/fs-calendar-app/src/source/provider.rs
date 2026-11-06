//! `CalendarAdapter` as the workbench sees it.
//!
//! Pattern: Adapter — each method of `CalendarSource` in terms of the contract `Calendar`. The form is
//! always of the stored event, and a save goes onto the event as it is stored now.

use std::collections::HashMap;
use std::collections::hash_map::Entry;

use chrono::Utc;
use fs_calendar_core::Event;
use fs_components_workbenches::SourceError;
use fs_components_workbenches::calendar::{
    CalendarCapabilities, CalendarSource, Day, ItemDraft, ItemProblem, ItemSheet, ItemsRead, Moment,
};
use fs_store::{ObjectId, StoredObject};

use super::adapter::{CalendarAdapter, event_of, reason, revision_of};
use super::apply::apply_draft;
use super::draft::{TITLE, blank_draft, draft_of, is_series};
use super::found::{CUT_AT, damaged};
use super::items::{is_within, item_of_instance, item_of_task};
use super::key::ItemKey;
use super::marks::is_marked;
use super::sheet::sheet_of;
use super::task_sheet::sheet_of_task;
use super::when::moment_of;
use crate::words::say;

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

impl CalendarSource for CalendarAdapter {
    fn capabilities(&self) -> CalendarCapabilities {
        let report = self.calendar.capabilities();
        let mut can = CalendarCapabilities::read_only();
        can.edit = report.is_supported("event.update");
        can.create = report.is_supported("event.create");
        can.delete = report.is_supported("event.delete");
        can
    }

    fn now(&self) -> Moment {
        self.frozen
            .unwrap_or_else(|| moment_of(self.zone.to_local(Utc::now())))
    }

    fn week_start(&self) -> u8 {
        self.week_start
    }

    fn items(&self, first: Day, last: Day) -> Result<ItemsRead, SourceError> {
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
                        problems.push(damaged(id, &id.to_string(), &e.message));
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
                        problems.push(damaged(id, &master.title, &why));
                    }
                }
            }
        }
        for (id, master) in &masters {
            if master.count >= LOOK_CLOSER_AT && self.cut_by_core(&master.event, first, last) {
                let text = say::TOO_MANY
                    .replace("{title}", &master.title)
                    .replace("{count}", &CUT_AT.to_string());
                problems.push(ItemProblem::of(id.to_string(), text));
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

    fn delete_warning(&self, key: &str) -> Option<String> {
        let series = match ItemKey::parse(key).ok()? {
            ItemKey::Task { .. } => false,
            ItemKey::Event {
                recurrence: Some(_),
                ..
            } => true,
            ItemKey::Event {
                id,
                recurrence: None,
            } => {
                let content = self.stored(id).ok()?.content;
                content.get("recurrenceRule").is_some()
                    || content.get("recurrenceOverrides").is_some()
            }
        };
        series.then(|| say::SERIES_DELETE.to_string())
    }

    fn read(&self, key: &str) -> Result<ItemSheet, SourceError> {
        match ItemKey::parse(key)? {
            ItemKey::Task { id, due } => {
                let agenda = self.found(due, due)?;
                agenda
                    .tasks
                    .iter()
                    .find(|entry| entry.id == id)
                    .and_then(|entry| sheet_of_task(entry, &self.zone))
                    .ok_or_else(|| SourceError::not_found("This task is no longer due here."))
            }
            ItemKey::Event { id, recurrence } => {
                let object = self.stored(id)?;
                let event = event_of(&object)?;
                let instance = recurrence
                    .map(|r| self.occurrence(id, &event, &r))
                    .transpose()?;
                sheet_of(&object, &event, instance.as_ref(), &self.zone)
                    .map_err(SourceError::failed)
            }
        }
    }

    /// Always the stored event, also when one occurrence was chosen.
    fn draft(&self, key: &str) -> Result<ItemDraft, SourceError> {
        let object = self.stored(ItemKey::event_id(key)?)?;
        let event = event_of(&object)?;
        draft_of(&event, Some(revision_of(&object)), is_marked(&object))
            .map_err(SourceError::failed)
    }

    fn blank(&self, day: Day) -> Result<ItemDraft, SourceError> {
        Ok(blank_draft(day, &self.zone))
    }

    /// The form goes onto the event as it is stored now, so whatever the form does not show stays.
    fn save(&self, key: &str, draft: &ItemDraft) -> Result<(), SourceError> {
        let id = ItemKey::event_id(key)?;
        let object = self.stored(id)?;
        if draft.revision.as_deref() != Some(revision_of(&object).as_str()) {
            return Err(SourceError::conflict(
                "Someone else changed this event since you started.",
            ));
        }
        let mut event = event_of(&object)?;
        apply_draft(&mut event, draft)?;
        self.early.forget();
        self.runtime
            .block_on(self.calendar.update(id, event))
            .map(|_| ())
            .map_err(|e| reason(&e))
    }

    fn create(&self, draft: &ItemDraft) -> Result<String, SourceError> {
        let mut event = Event::new("", "1970-01-01T00:00:00", "Etc/UTC", "PT0S");
        event.title = None;
        event.time_zone = None;
        event.duration = None;
        // Everything in the form counts as written, what it held at the start too.
        let mut written = draft.clone();
        for field in &mut written.fields {
            field.original = String::new();
        }
        if draft.value(TITLE).is_none_or(|t| t.trim().is_empty()) {
            return Err(SourceError::failed(say::NEEDS_TITLE));
        }
        apply_draft(&mut event, &written)?;
        self.early.forget();
        self.runtime
            .block_on(self.calendar.create(event))
            .map(|object| object.envelope.id.to_string())
            .map_err(|e| reason(&e))
    }

    fn delete(&self, key: &str) -> Result<(), SourceError> {
        self.early.forget();
        self.runtime
            .block_on(self.calendar.delete(ItemKey::event_id(key)?))
            .map_err(|e| reason(&e))
    }
}
