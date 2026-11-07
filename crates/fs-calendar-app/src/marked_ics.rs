//! iCalendar that keeps what is marked.
//!
//! Pattern: Decorator over the core's `IcsFormat` — the export says private for what was released,
//! with its categories, so the file carries the marking the envelope cannot give it.

use std::collections::HashMap;

use fs_calendar_core::{Event, Exporter, IcsFormat};
use fs_objects::StoredObject;
use serde_json::json;

use crate::source::{SENSITIVE_KEY, is_marked};
use crate::transfer::uid_of;

/// `uid` and categories of what is marked among `chosen`, the markings to carry into the file.
pub(crate) fn marked_only_outside(chosen: &[&StoredObject]) -> HashMap<String, Vec<String>> {
    chosen
        .iter()
        .filter(|object| is_marked(object))
        .map(|object| {
            let categories = object.envelope.sensitive.iter().map(ToString::to_string);
            (uid_of(object).to_string(), categories.collect())
        })
        .collect()
}

/// iCalendar with the markings kept: a released event leaves as private, with its categories, also if
/// the marking was on the envelope, which the file has no place for.
pub(crate) struct MarkedIcs(pub(crate) HashMap<String, Vec<String>>);

impl Exporter for MarkedIcs {
    fn output_media_type(&self) -> &'static str {
        IcsFormat.output_media_type()
    }

    fn export(&self, objects: &[Event]) -> fs_calendar_core::Result<String> {
        let mut marked = objects.to_vec();
        for event in &mut marked {
            let Some(categories) = self.0.get(&event.uid) else {
                continue;
            };
            if event.privacy.as_deref().is_none_or(|p| p == "public") {
                event.privacy = Some("private".to_string());
            }
            if !categories.is_empty() && event.vendor(SENSITIVE_KEY).is_none() {
                event.set_vendor(SENSITIVE_KEY, json!(categories));
            }
        }
        IcsFormat.export(&marked)
    }
}
