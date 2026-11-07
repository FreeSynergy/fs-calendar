//! A task to be read.
//!
//! Pattern: Adapter — read through the tasks contract, never owned or changed here (ADR 0050).

use fs_calendar_client::TaskLine;
use fs_calendar_core::time::Zone;
use fs_components_workbenches::calendar::{ItemSheet, SheetField, SheetSection};

use super::items::item_of_task;
use super::marks::content_is_marked;
use crate::words::{field, say};

/// A task to be read: never changed here.
pub(super) fn sheet_of_task(entry: &TaskLine, zone: &Zone) -> Option<ItemSheet> {
    let content = &entry.content;
    let item = item_of_task(entry, zone)?;
    let mut fields = vec![SheetField::new(
        field::DUE,
        if item.all_day {
            item.start.day.to_string()
        } else {
            format!("{} {}", item.start.day, item.start.clock())
        },
    )];
    if let Some(progress) = content["progress"].as_str() {
        fields.push(SheetField::new(field::PROGRESS, progress));
    }
    if let Some(text) = content["description"].as_str().filter(|t| !t.is_empty()) {
        fields.push(SheetField::new(field::DESCRIPTION, text));
    }
    let sections = vec![SheetSection::new(field::TASK, fields)];
    let mut sheet = ItemSheet::new(entry.name.clone(), item.start.day, sections);
    sheet.marks.push(field::TASK.to_string());
    if content_is_marked(content) {
        sheet.marks.push(field::SENSITIVE.to_string());
    }
    sheet.note = Some(say::TASK_NOTE.to_string());
    sheet.can_edit = false;
    sheet.can_delete = false;
    Some(sheet)
}
