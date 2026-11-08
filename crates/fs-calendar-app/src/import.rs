//! iCalendar files in, by a path from the command line.
//!
//! Pattern: Template method over the contract — fixed flow: read, import, report. Nothing is hidden:
//! what failed, or lost its time zone on the way, is in the report.

use std::fmt::Write as _;
use std::path::Path;

use fs_calendar_core::{Calendar, IcsFormat, ImportReport, Importer};
use fs_objects::ObjectId;
use tokio::runtime::Handle;

use crate::lost_zones::lost_zones;
use crate::source::{caught, reason_text};
use crate::words::say;

/// What an import did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ImportSummary {
    /// Objects that were added.
    pub created: usize,
    /// Objects that were known already and were left as they are.
    pub skipped: usize,
    /// What could not be imported, with the reason.
    pub failed: Vec<String>,
    /// What the core has to say about the import, such as tasks left out.
    pub notes: Vec<String>,
    /// Events stored without the zone the file gave them, so they now mean another time.
    pub zone_lost: Vec<String>,
}

impl ImportSummary {
    /// Did all of it work?
    pub fn is_clean(&self) -> bool {
        self.failed.is_empty() && self.zone_lost.is_empty()
    }

    /// In words, for the status line.
    pub fn message(&self) -> String {
        let mut text = format!("Imported {}, {} already known", self.created, self.skipped);
        if !self.failed.is_empty() {
            let _ = write!(
                text,
                ", {} failed: {}",
                self.failed.len(),
                self.failed.join("; ")
            );
        }
        if !self.zone_lost.is_empty() {
            let _ = write!(
                text,
                ". Their time zone is unknown, so they are kept as clock times without one and may mean another time: {}",
                self.zone_lost.join("; ")
            );
        }
        if !self.notes.is_empty() {
            let _ = write!(text, ". Notes: {}", self.notes.join("; "));
        }
        text
    }
}

/// Import the iCalendar file at `path`.
///
/// # Errors
/// A message for the person if the file cannot be read or holds no calendar.
pub fn import_file(
    calendar: &dyn Calendar,
    runtime: &Handle,
    path: &Path,
) -> Result<ImportSummary, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    if !text.to_ascii_uppercase().contains("BEGIN:VCALENDAR") {
        return Err(format!("{} holds no calendar", path.display()));
    }
    let mut summary = ImportSummary::default();
    // One import of the whole text: the core reads the known `uid`s once, so a file of n parts costs n,
    // not n times the events already stored. Only if that panics (the time library cannot hold a part)
    // is it done part by part, so that part costs itself, named by `uid`.
    let mut stored = Vec::new();
    match caught(|| runtime.block_on(calendar.import_ics(&text))) {
        Ok(imported) => {
            let mut ids = Vec::new();
            take(&mut summary, "the file", Ok(imported), &mut |id| {
                ids.push(id);
            });
            stored = originals(calendar, runtime, &ids);
        }
        Err(_) => import_by_part(calendar, runtime, &text, &mut summary, &mut stored)?,
    }
    summary.zone_lost = lost_zones(calendar, runtime, &stored);
    Ok(summary)
}

fn take(
    summary: &mut ImportSummary,
    uid: &str,
    imported: Result<fs_calendar_core::Result<ImportReport>, String>,
    created: &mut dyn FnMut(ObjectId),
) {
    match imported {
        Ok(Ok(report)) => {
            summary.created += report.created.len();
            report.created.into_iter().for_each(created);
            summary.skipped += report.skipped.len();
            summary.failed.extend(report.failed);
            summary.notes.extend(report.notes);
        }
        Ok(Err(e)) => summary.failed.push(format!("{uid}: {}", reason_text(&e))),
        Err(_) => summary.failed.push(format!("{uid}: {}", say::IMPORT_PANIC)),
    }
}

/// The text each stored event came from (the part of the file the core kept as its original), by id.
/// A task has none here: it is not an event of this calendar.
fn originals(
    calendar: &dyn Calendar,
    runtime: &Handle,
    ids: &[ObjectId],
) -> Vec<(ObjectId, String)> {
    ids.iter()
        .filter_map(|id| {
            let object = runtime.block_on(calendar.get(*id)).ok().flatten()?;
            let bytes = object.original?.bytes;
            Some((*id, String::from_utf8(bytes).ok()?))
        })
        .collect()
}

/// The way out after a panic: one part at a time, events first and then tasks.
fn import_by_part(
    calendar: &dyn Calendar,
    runtime: &Handle,
    text: &str,
    summary: &mut ImportSummary,
    stored: &mut Vec<(ObjectId, String)>,
) -> Result<(), String> {
    for part in IcsFormat.split(text).map_err(|e| reason_text(&e))?.parts {
        let uid = part.uid.clone().unwrap_or_default();
        let imported = caught(|| runtime.block_on(calendar.import(&IcsFormat, &part.text)));
        take(summary, &uid, imported, &mut |id| {
            stored.push((id, part.text.clone()));
        });
    }
    for part in fs_tasks_core::ical::split_ics(text) {
        let imported = caught(|| runtime.block_on(calendar.import_ics(&part.text)));
        take(summary, &part.uid, imported, &mut |_| {});
    }
    Ok(())
}
