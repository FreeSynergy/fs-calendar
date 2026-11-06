//! Import and export through the format traits.
//!
//! Pattern: Template method — `import` is the fixed flow (split, skip known uid, parse, normalise,
//! store, report); splitting and parsing are the [`Importer`] strategy. `import_ics` and friends
//! are thin conveniences for the shipped formats.

use fs_store::{Blob, ObjectId};

use super::handle::CalendarRepo;
use super::types::ImportReport;
use crate::error::{Error, Result};
use crate::format::{Exporter, IcsFormat, Importer, JsCalendarFormat};
use crate::model::CalendarObject;

impl CalendarRepo {
    /// Import the objects `importer` finds in `text`. Known `uid`s are skipped, never
    /// overwritten; the importer's original text (ADR 0030) is kept per part, and the tz database
    /// version used is recorded in the index (`tzdb_version`).
    pub async fn import(&self, importer: &dyn Importer, text: &str) -> Result<ImportReport> {
        let split = importer.split(text)?;
        let mut report = ImportReport {
            notes: split.notes,
            ..Default::default()
        };
        for part in split.parts {
            if let Some(uid) = &part.uid {
                if self.get_by_uid(uid).await?.is_some() {
                    report.skipped.push(uid.clone());
                    continue;
                }
            }
            match importer.parse(&part) {
                Ok(parsed) => {
                    report.notes.extend(parsed.notes);
                    for obj in parsed.objects {
                        self.import_one(obj, parsed.original.clone(), &mut report)
                            .await?;
                    }
                }
                Err(e) => report.failed.push(match &part.uid {
                    Some(uid) => format!("{uid}: {e}"),
                    None => e.to_string(),
                }),
            }
        }
        Ok(report)
    }

    /// Import JSCalendar JSON (object, array, or `Group`) in 1.0 or 2.0 form; 1.0 is upgraded
    /// first.
    pub async fn import_jscalendar(&self, json_text: &str) -> Result<ImportReport> {
        self.import(&JsCalendarFormat, json_text).await
    }

    /// Import an `.ics` text. Each UID gets its own original (`text/calendar`, ADR 0030), and the
    /// tz database version used is recorded in the index (`tzdb_version`).
    pub async fn import_ics(&self, text: &str) -> Result<ImportReport> {
        self.import(&IcsFormat, text).await
    }

    async fn import_one(
        &self,
        mut obj: CalendarObject,
        original: Option<Blob>,
        report: &mut ImportReport,
    ) -> Result<()> {
        match &mut obj {
            CalendarObject::Event(e) => e.normalise(),
            CalendarObject::Task(t) => t.normalise(),
        }
        if obj.uid().is_empty() {
            report.failed.push("component without uid".into());
            return Ok(());
        }
        if self.get_by_uid(obj.uid()).await?.is_some() {
            report.skipped.push(obj.uid().to_string());
            return Ok(());
        }
        let uid = obj.uid().to_string();
        match self.create_inner(obj, original).await {
            Ok(o) => report.created.push(o.envelope.id),
            Err(e @ (Error::Invalid(_) | Error::Time(_) | Error::Recurrence(_))) => {
                report.failed.push(format!("{uid}: {e}"));
            }
            Err(e) => return Err(e),
        }
        Ok(())
    }

    /// Export objects as text of the format `exporter` writes.
    pub async fn export(&self, exporter: &dyn Exporter, ids: &[ObjectId]) -> Result<String> {
        let mut objs = vec![];
        for id in ids {
            objs.push(self.get_object(*id).await?.ok_or(Error::NotFound(*id))?);
        }
        exporter.export(&objs)
    }

    /// Export objects as iCalendar text.
    pub async fn export_ics(&self, ids: &[ObjectId]) -> Result<String> {
        self.export(&IcsFormat, ids).await
    }
}
