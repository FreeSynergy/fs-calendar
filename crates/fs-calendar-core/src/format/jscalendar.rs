//! JSCalendar JSON as an [`Importer`] and [`Exporter`].
//!
//! Pattern: Adapter — the native wire form (one object, an array, or a `Group`); RFC 8984 (1.0)
//! data is upgraded first, and the upgrade notes are reported. Export writes the object itself
//! for one event and a JSON array for several; both read back through the importer.

use fs_tasks_core::Task;
use serde_json::{Value, json};

use super::media::JSCALENDAR_MEDIA_TYPE;
use super::traits::{Exporter, Importer, Parsed, Part, Split};
use crate::error::Result;
use crate::model::Event;
use crate::upgrade::upgrade;

/// JSCalendar JSON in 1.0 or 2.0 form.
#[derive(Clone, Copy, Debug, Default)]
pub struct JsCalendarFormat;

impl Importer for JsCalendarFormat {
    fn media_type(&self) -> &'static str {
        JSCALENDAR_MEDIA_TYPE
    }

    fn split(&self, text: &str) -> Result<Split> {
        let up = upgrade(match serde_json::from_str::<Value>(text)? {
            Value::Array(a) => json!({"@type": "Group", "entries": a}),
            other => other,
        });
        let entries = match up.value.get("entries") {
            Some(Value::Array(a)) => a.clone(),
            _ => vec![up.value],
        };
        let parts = entries
            .into_iter()
            .map(|e| Part {
                uid: None,
                text: e.to_string(),
            })
            .collect();
        Ok(Split {
            parts,
            notes: up.notes,
        })
    }

    fn parse(&self, part: &Part) -> Result<Parsed> {
        let value: Value = serde_json::from_str(&part.text)?;
        let is_task = value.get("@type").and_then(Value::as_str) == Some(Task::TYPE);
        let (objects, tasks) = if is_task {
            (vec![], vec![Task::from_value(value)?])
        } else {
            (vec![Event::from_value(value)?], vec![])
        };
        Ok(Parsed {
            objects,
            tasks,
            original: None,
            notes: vec![],
        })
    }
}

impl Exporter for JsCalendarFormat {
    fn output_media_type(&self) -> &'static str {
        JSCALENDAR_MEDIA_TYPE
    }

    fn export(&self, objects: &[Event]) -> Result<String> {
        let value = match objects {
            [one] => one.to_value(),
            many => Value::Array(many.iter().map(Event::to_value).collect()),
        };
        Ok(serde_json::to_string(&value)?)
    }
}
