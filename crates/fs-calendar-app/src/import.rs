//! iCalendar files in, by a path from the command line.
//!
//! Pattern: Template method over the contract — fixed flow: read, import, report. Nothing is hidden:
//! what failed, or lost its time zone on the way, is in the report.

use std::fmt::Write as _;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;

use fs_calendar_core::Calendar;
use tokio::runtime::Handle;

use crate::lost_zones::lost_zones;
use crate::source::reason_text;
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
    /// The import stopped here (what came before is in).
    pub stopped: Option<String>,
}

impl ImportSummary {
    /// Did all of it work?
    pub fn is_clean(&self) -> bool {
        self.failed.is_empty() && self.zone_lost.is_empty() && self.stopped.is_none()
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
        if let Some(why) = &self.stopped {
            let _ = write!(text, ". The import stopped: {why}");
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
    // The time library of the core can panic on a far too long length; that must not take the program.
    let imported = catch_unwind(AssertUnwindSafe(|| {
        runtime.block_on(calendar.import_ics(&text))
    }));
    match imported {
        Ok(Ok(report)) => {
            summary.created = report.created.len();
            summary.skipped = report.skipped.len();
            summary.failed = report.failed;
            summary.notes = report.notes;
        }
        Ok(Err(e)) => summary.stopped = Some(reason_text(&e)),
        Err(_) => summary.stopped = Some(say::IMPORT_PANIC.to_string()),
    }
    summary.zone_lost = lost_zones(calendar, runtime, &text);
    Ok(summary)
}
