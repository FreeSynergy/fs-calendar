//! A task program for tests: it answers from a list, remembers what it was asked, and stores nothing
//! but the `uid`s it was handed.

#![allow(dead_code)] // each test file uses a different subset

use std::sync::Mutex;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use fs_calendar_core::{DueTask, Forwarded, ImportReport, TaskIntake, TasksError, TasksInWindow};
use fs_objects::{Envelope, ObjectId};

/// Answers every window with `due`, refuses a task with `percentComplete` above 100, skips a `uid` it
/// has, and fails every call if `broken`.
#[derive(Default)]
pub struct FakeTasks {
    pub due: Vec<DueTask>,
    pub broken: bool,
    pub windows: Mutex<Vec<(DateTime<Utc>, DateTime<Utc>)>>,
    pub uids: Mutex<Vec<String>>,
    pub texts: Mutex<Vec<String>>,
}

impl FakeTasks {
    /// A task due somewhere, titled `title`.
    pub fn task(title: &str) -> DueTask {
        DueTask {
            id: ObjectId::new(),
            envelope: Envelope::new("Task", title),
            jscalendar: serde_json::json!({"@type": "Task", "title": title}),
        }
    }

    fn take(&self, uid: &str) -> Option<ObjectId> {
        let mut uids = self.uids.lock().unwrap();
        if uids.iter().any(|u| u == uid) {
            return None;
        }
        uids.push(uid.to_string());
        Some(ObjectId::new())
    }
}

#[async_trait]
impl TasksInWindow for FakeTasks {
    async fn due_between(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<DueTask>, TasksError> {
        if self.broken {
            return Err(TasksError::new("the task program is gone"));
        }
        self.windows.lock().unwrap().push((from, to));
        Ok(self.due.clone())
    }
}

#[async_trait]
impl TaskIntake for FakeTasks {
    async fn import_ics(&self, text: &str) -> Result<ImportReport, TasksError> {
        self.texts.lock().unwrap().push(text.to_string());
        let mut report = ImportReport::default();
        let mut in_todo = false;
        for line in text.lines() {
            match line.trim() {
                "BEGIN:VTODO" => in_todo = true,
                "END:VTODO" => in_todo = false,
                l if in_todo && l.starts_with("UID:") => {
                    let uid = &l[4..];
                    match self.take(uid) {
                        Some(id) => report.created.push(id),
                        None => report.skipped.push(uid.to_string()),
                    }
                }
                _ => {}
            }
        }
        Ok(report)
    }

    async fn forward(&self, uid: &str, jscalendar: &str) -> Result<Forwarded, TasksError> {
        if self.broken {
            return Err(TasksError::new("the task program is gone"));
        }
        self.texts.lock().unwrap().push(jscalendar.to_string());
        let task: serde_json::Value = serde_json::from_str(jscalendar).unwrap();
        if task["percentComplete"].as_u64().is_some_and(|p| p > 100) {
            return Ok(Forwarded::Refused(
                "percentComplete must be 0 to 100".into(),
            ));
        }
        Ok(self
            .take(uid)
            .map_or(Forwarded::Skipped, Forwarded::Created))
    }
}
