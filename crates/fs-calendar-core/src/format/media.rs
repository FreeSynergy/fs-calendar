//! Media type to format: the choice behind `importText` / `exportText` of `events.capnp`.
//!
//! Pattern: Factory — a media type string picks the format; an unknown one is an error, never a
//! guess. Parameters (`; charset=utf-8`) and case are ignored, as in HTTP.

use super::{Exporter, IcsFormat, Importer, JsCalendarFormat};
use crate::error::{Error, Result};
use crate::ical::ICS_MEDIA_TYPE;

/// Media type of JSCalendar JSON.
pub const JSCALENDAR_MEDIA_TYPE: &str = "application/jscalendar+json";

/// The text formats a calendar reads and writes by media type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextFormat {
    /// iCalendar (`text/calendar`).
    Ics,
    /// JSCalendar JSON (`application/jscalendar+json`).
    JsCalendar,
}

impl TextFormat {
    /// The format for `media_type`.
    ///
    /// # Errors
    /// [`Error::MediaType`] when no format is known for it.
    pub fn from_media_type(media_type: &str) -> Result<Self> {
        let essence = media_type
            .split(';')
            .next()
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase();
        match essence.as_str() {
            ICS_MEDIA_TYPE => Ok(Self::Ics),
            JSCALENDAR_MEDIA_TYPE => Ok(Self::JsCalendar),
            _ => Err(Error::MediaType(media_type.to_string())),
        }
    }

    /// The importer of this format.
    pub fn importer(self) -> &'static dyn Importer {
        match self {
            Self::Ics => &IcsFormat,
            Self::JsCalendar => &JsCalendarFormat,
        }
    }

    /// The exporter of this format.
    pub fn exporter(self) -> &'static dyn Exporter {
        match self {
            Self::Ics => &IcsFormat,
            Self::JsCalendar => &JsCalendarFormat,
        }
    }
}
