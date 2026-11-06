//! A task to be read.
//!
//! Pattern: Adapter — read through the `fs-tasks` interface, never owned or changed here (ADR 0050).

use fs_calendar_core::time::Zone;
use fs_components_workbenches::calendar::{ItemSheet, SheetField, SheetSection};
use fs_tasks_core::TaskEntry;

use super::items::item_of_task;
use super::marks::content_is_marked;
use crate::words::{field, say};

/// A task to be read: never changed here.
pub(super) fn sheet_of_task(entry: &TaskEntry, zone: &Zone) -> Option<ItemSheet> {
    let task = &entry.task;
    let item = item_of_task(entry, zone)?;
    let mut fields = vec![SheetField::new(
        field::DUE,
        if item.all_day {
            item.start.day.to_string()
        } else {
            format!("{} {}", item.start.day, item.start.clock())
        },
    )];
    if let Some(progress) = &task.progress {
        fields.push(SheetField::new(field::PROGRESS, progress.clone()));
    }
    if let Some(text) = task.description.as_deref().filter(|t| !t.is_empty()) {
        fields.push(SheetField::new(field::DESCRIPTION, text));
    }
    let sections = vec![SheetSection::new(field::TASK, fields)];
    let mut sheet = ItemSheet::new(task.display_title(), item.start.day, sections);
    sheet.marks.push(field::TASK.to_string());
    if content_is_marked(&task.to_value()) {
        sheet.marks.push(field::SENSITIVE.to_string());
    }
    sheet.note = Some(say::TASK_NOTE.to_string());
    sheet.can_edit = false;
    sheet.can_delete = false;
    Some(sheet)
}
