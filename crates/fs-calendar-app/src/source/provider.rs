//! `CalendarAdapter` as the workbench sees it.
//!
//! Pattern: Adapter — each method of `CalendarSource` in terms of the contract `Calendar`. The form is
//! always of the stored event, and a save goes onto the event as it is stored now.

use std::collections::HashMap;

use chrono::Utc;
use fs_calendar_core::Event;
use fs_components_workbenches::SourceError;
use fs_components_workbenches::calendar::{
    CalendarCapabilities, CalendarItem, CalendarSource, Day, ItemDraft, ItemSheet, Moment,
};
use fs_store::{ObjectId, StoredObject};

use super::adapter::{CalendarAdapter, event_of, reason, revision_of};
use super::apply::apply_draft;
use super::draft::{TITLE, blank_draft, draft_of, is_series};
use super::items::{is_within, item_of_instance, item_of_task};
use super::key::ItemKey;
use super::marks::is_marked;
use super::sheet::{sheet_of, sheet_of_task};
use super::when::moment_of;
use crate::words::say;

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

    fn items(&self, first: Day, last: Day) -> Result<Vec<CalendarItem>, SourceError> {
        let agenda = self.agenda(first, last)?;
        let mut masters: HashMap<ObjectId, StoredObject> = HashMap::new();
        let mut items = Vec::new();
        for instance in &agenda.events {
            if !masters.contains_key(&instance.object) {
                let object = self.stored(instance.object)?;
                masters.insert(instance.object, object);
            }
            let object = &masters[&instance.object];
            let series = is_series(&event_of(object)?);
            let key = if series {
                format!("{}#{}", instance.object, instance.occurrence.recurrence_id)
            } else {
                instance.object.to_string()
            };
            let item = item_of_instance(instance, key, &self.zone, is_marked(object))
                .map_err(SourceError::failed)?;
            if is_within(&item, first, last) {
                items.push(item);
            }
        }
        for entry in &agenda.tasks {
            if let Some(item) =
                item_of_task(entry, &self.zone).filter(|i| is_within(i, first, last))
            {
                items.push(item);
            }
        }
        Ok(items)
    }

    fn read(&self, key: &str) -> Result<ItemSheet, SourceError> {
        match ItemKey::parse(key)? {
            ItemKey::Task { id, due } => {
                let agenda = self.agenda(due, due)?;
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
        self.runtime
            .block_on(self.calendar.create(event))
            .map(|object| object.envelope.id.to_string())
            .map_err(|e| reason(&e))
    }

    fn delete(&self, key: &str) -> Result<(), SourceError> {
        self.runtime
            .block_on(self.calendar.delete(ItemKey::event_id(key)?))
            .map_err(|e| reason(&e))
    }
}
