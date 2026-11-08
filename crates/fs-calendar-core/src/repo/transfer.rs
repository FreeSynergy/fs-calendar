//! Import and export of events through the format traits.
//!
//! Pattern: Template method — `import` is the fixed flow (split, skip known uid, parse, normalise,
//! store, report; the known `uid`s are read once, see `known.rs`); splitting and parsing are the [`Importer`] strategy. `import_ics` and friends
//! are thin conveniences for the shipped formats.

use fs_jscalendar::ical::{Component, split_ics};
use fs_objects::{Blob, ObjectId};

use super::handle::CalendarRepo;
use super::known::KnownUids;
use super::types::ImportReport;
use crate::error::{Error, Result};
use crate::format::{Exporter, IcsFormat, Importer, JsCalendarFormat, TaskPart};
use crate::model::Event;
use crate::tasks::Forwarded;
use crate::uid::normalise_uid;

impl CalendarRepo {
    /// Import the objects `importer` finds in `text`. Known `uid`s are skipped, never
    /// overwritten; the importer's original text (ADR 0030) is kept per part, and the tz database
    /// version used is recorded in the index (`tzdb_version`).
    pub async fn import(&self, importer: &dyn Importer, text: &str) -> Result<ImportReport> {
        let _one_at_a_time = self.writes.lock().await;
        let split = importer.split(text)?;
        // Asked once: a lookup by `uid` for each part would look at every stored event each time.
        // (Nothing to read if there is nothing to import.)
        let mut known = if split.parts.is_empty() {
            Default::default()
        } else {
            self.known_uids().await?
        };
        let mut report = ImportReport {
            notes: split.notes,
            ..Default::default()
        };
        for part in split.parts {
            if let Some(uid) = &part.uid {
                if known.contains(uid) {
                    report.skipped.push(uid.clone());
                    continue;
                }
            }
            match importer.parse(&part) {
                Ok(parsed) => {
                    report.notes.extend(parsed.notes);
                    for obj in parsed.objects {
                        self.import_one(obj, parsed.original.clone(), &mut known, &mut report)
                            .await?;
                    }
                    for task in parsed.tasks {
                        self.forward_task(task, &mut report).await?;
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

    /// Import the `VEVENT`s of an `.ics` text. Each UID gets its own original (`text/calendar`,
    /// ADR 0030), and the tz database version used is recorded in the index (`tzdb_version`).
    ///
    /// `VTODO`s are not ours (ADR 0050): if a task program is attached ([`CalendarRepo::with_task_intake`])
    /// the text is handed to its `import_ics`, and the report covers both; otherwise the report
    /// notes each of them as not imported.
    pub async fn import_ics(&self, text: &str) -> Result<ImportReport> {
        let mut report = self.import(&IcsFormat, text).await?;
        if let Some(intake) = &self.intake {
            let t = intake.import_ics(text).await?;
            report.created.extend(t.created);
            report.skipped.extend(t.skipped);
            report.failed.extend(t.failed);
            report.notes.extend(t.notes);
        } else {
            for part in split_ics(text, Component::Task) {
                report.notes.push(no_task_source(&part.uid));
            }
        }
        Ok(report)
    }

    /// Hand a task found by an importer to the task program (ADR 0050). Known `uid`s are skipped by
    /// the task program, and without one the report notes that the task was not imported.
    async fn forward_task(&self, task: TaskPart, report: &mut ImportReport) -> Result<()> {
        let Some(intake) = &self.intake else {
            report.notes.push(no_task_source(&task.uid));
            return Ok(());
        };
        // The uid goes on as written: the task program keeps and matches it as written, so a second
        // import finds the task again whatever the spelling (upper-case UUID, padding).
        let uid = task.uid;
        if normalise_uid(&uid).is_empty() {
            report.failed.push("component without uid".into());
            return Ok(());
        }
        match intake.forward(&uid, &task.jscalendar).await? {
            Forwarded::Created(id) => report.created.push(id),
            Forwarded::Skipped => report.skipped.push(uid),
            Forwarded::Refused(why) => report.failed.push(format!("{uid}: {why}")),
        }
        Ok(())
    }

    async fn import_one(
        &self,
        mut obj: Event,
        original: Option<Blob>,
        known: &mut KnownUids,
        report: &mut ImportReport,
    ) -> Result<()> {
        obj.normalise();
        if obj.uid.is_empty() {
            report.failed.push("component without uid".into());
            return Ok(());
        }
        if known.contains(&obj.uid) {
            report.skipped.push(obj.uid.clone());
            return Ok(());
        }
        let uid = obj.uid.clone();
        match self.store_new(obj, original).await {
            Ok(o) => {
                known.add(&uid);
                report.created.push(o.envelope.id);
            }
            Err(
                e @ (Error::Invalid(_)
                | Error::Time(_)
                | Error::Recurrence(_)
                | Error::RuleTooExpensive(_)
                | Error::TooHeavy(_)
                | Error::Limit(_)
                | Error::TooLarge(_)),
            ) => {
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
            objs.push(self.get_event(*id).await?.ok_or(Error::NotFound(*id))?);
        }
        exporter.export(&objs)
    }

    /// Export events as iCalendar text (tasks are exported by `fs-tasks`).
    pub async fn export_ics(&self, ids: &[ObjectId]) -> Result<String> {
        self.export(&IcsFormat, ids).await
    }
}

fn no_task_source(uid: &str) -> String {
    format!("{uid}: not imported, it is a task and no task program is attached")
}
