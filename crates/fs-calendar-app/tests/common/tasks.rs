//! A fixed list of tasks behind the narrow `TaskLines` trait.
//!
//! Pattern: none — a test helper: the tasks program is another program, the app sees only the
//! trait, and the wire to the real tasks server is proven in `fs-calendar-client`. This list answers
//! what is due in a window, like the server, and records the windows it was asked about.

use std::sync::Mutex;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use fs_calendar_client::{TaskLine, TaskLines, TasksUnread};
use fs_calendar_core::time::{Zone, parse_local};
use fs_store::ObjectId;
use serde_json::{Value, json};

/// The tasks, and what was asked.
#[derive(Default)]
pub struct FixedTasks {
    lines: Mutex<Vec<TaskLine>>,
    pub asked: Mutex<Vec<(DateTime<Utc>, DateTime<Utc>)>>,
    pub failing: Mutex<Option<String>>,
}

impl FixedTasks {
    /// A task due at this local time in Berlin (none = undated); its id as text.
    pub fn add(&self, title: &str, due: Option<&str>) -> String {
        let mut content = json!({"@type": "Task", "version": "2.0", "uid": title, "title": title});
        if let Some(due) = due {
            content["due"] = json!(due);
            content["timeZone"] = json!("Europe/Berlin");
        }
        self.add_content(title, content)
    }

    pub fn add_content(&self, title: &str, content: Value) -> String {
        let id = ObjectId::new();
        self.lines.lock().unwrap().push(TaskLine {
            id,
            name: title.to_string(),
            content,
        });
        id.to_string()
    }

    pub fn count(&self) -> usize {
        self.lines.lock().unwrap().len()
    }

    /// The source cannot be read from now on.
    pub fn fail_with(&self, why: &str) {
        *self.failing.lock().unwrap() = Some(why.to_string());
    }
}

#[async_trait]
impl TaskLines for FixedTasks {
    async fn due_between(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<TaskLine>, TasksUnread> {
        self.asked.lock().unwrap().push((from, to));
        if let Some(why) = self.failing.lock().unwrap().clone() {
            return Err(TasksUnread(why));
        }
        let due_in_window = |line: &TaskLine| {
            let due = line.content["due"]
                .as_str()
                .and_then(|d| parse_local(d).ok())?;
            let zone = Zone::parse(line.content["timeZone"].as_str().unwrap_or("Etc/UTC")).ok()?;
            let at = zone.to_utc(due);
            Some(at >= from && at < to)
        };
        let lines = self.lines.lock().unwrap();
        Ok(lines
            .iter()
            .filter(|l| due_in_window(l) == Some(true))
            .cloned()
            .collect())
    }
}
