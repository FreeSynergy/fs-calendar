//! iCalendar as an [`Importer`] / [`Exporter`].
//!
//! Pattern: Adapter — [`IcsFormat`] puts the `calcard` based `ical` module behind our traits.
//! Each UID is its own part with its own original (`text/calendar`, ADR 0030); the tz database
//! version in use is recorded by the repository.

use fs_store::Blob;

use super::traits::{Exporter, Importer, Parsed, Part, Split};
use crate::error::{Error, Result};
use crate::ical::{ICS_MEDIA_TYPE, parse_ics, split_ics, to_ics, unknown_zone};
use crate::model::Event;

/// iCalendar (RFC 5545) in and out.
#[derive(Clone, Copy, Debug, Default)]
pub struct IcsFormat;

impl Importer for IcsFormat {
    fn media_type(&self) -> &'static str {
        ICS_MEDIA_TYPE
    }

    fn split(&self, text: &str) -> Result<Split> {
        let parts = split_ics(text)
            .into_iter()
            .map(|p| Part {
                uid: Some(p.uid),
                text: p.text,
            })
            .collect();
        Ok(Split {
            parts,
            notes: vec![],
        })
    }

    fn parse(&self, part: &Part) -> Result<Parsed> {
        let (objects, notes) = parse_ics(&part.text)?;
        if let Some(name) = unknown_zone(&part.text) {
            // Stored, the event would mean another time: its zone is lost in the conversion.
            return Err(Error::ICalendar(format!(
                "the time zone {name:?} is unknown, so the event is not stored: it would mean another time"
            )));
        }
        let original = Blob {
            media_type: ICS_MEDIA_TYPE.into(),
            bytes: part.text.clone().into_bytes(),
        };
        Ok(Parsed {
            objects,
            // `VTODO`s keep their own original: `import_ics` hands the whole text to fs-tasks.
            tasks: vec![],
            original: Some(original),
            notes,
        })
    }
}

impl Exporter for IcsFormat {
    fn output_media_type(&self) -> &'static str {
        ICS_MEDIA_TYPE
    }

    fn export(&self, objects: &[Event]) -> Result<String> {
        to_ics(objects)
    }
}
