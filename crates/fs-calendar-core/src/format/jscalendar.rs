//! JSCalendar JSON as an [`Importer`].
//!
//! Pattern: Adapter — the native wire form (one object, an array, or a `Group`); RFC 8984 (1.0)
//! data is upgraded first, and the upgrade notes are reported.

use serde_json::{Value, json};

use super::traits::{Importer, Parsed, Part, Split};
use crate::error::Result;
use crate::model::CalendarObject;
use crate::upgrade::upgrade;

/// JSCalendar JSON in 1.0 or 2.0 form.
#[derive(Clone, Copy, Debug, Default)]
pub struct JsCalendarFormat;

impl Importer for JsCalendarFormat {
    fn media_type(&self) -> &'static str {
        "application/jscalendar+json"
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
        let object = CalendarObject::from_value(serde_json::from_str(&part.text)?)?;
        Ok(Parsed {
            objects: vec![object],
            original: None,
            notes: vec![],
        })
    }
}
