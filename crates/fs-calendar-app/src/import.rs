//! iCalendar files in, by a path from the command line.
//!
//! Pattern: Template method over the contract — fixed flow: read, import, report. Nothing is hidden:
//! what failed, or lost its time zone on the way, is in the report.

use std::fmt::Write as _;
use std::path::Path;

use fs_calendar_core::{Calendar, IcsFormat, ImportReport, Importer};
use fs_jscalendar::ical::{Component, split_ics};
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
    let mut stored = Vec::new();
    // One part at a time: a part the time library cannot hold costs that part, by its `uid`.
    for part in IcsFormat.split(&text).map_err(|e| reason_text(&e))?.parts {
        let uid = part.uid.clone().unwrap_or_default();
        let imported = caught(|| runtime.block_on(calendar.import(&IcsFormat, &part.text)));
        take(&mut summary, &uid, imported, &mut |id| {
            stored.push((id, part.text.clone()));
        });
    }
    // Tasks are not ours (ADR 0050): `import_ics` hands them on, one at a time too.
    for part in split_ics(&text, Component::Task) {
        let imported = caught(|| runtime.block_on(calendar.import_ics(&part.text)));
        take(&mut summary, &part.uid, imported, &mut |_| {});
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
