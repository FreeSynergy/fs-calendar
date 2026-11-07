//! Constants of the calendar objects.
//!
//! Pattern: Constants — the AS2 types, context and media type used by this program in one place.

/// `context` of all objects of this program.
pub const CONTEXT: &str = "calendar";
/// Media type of the object content.
pub const CONTENT_TYPE: &str = "application/jscalendar+json";
/// AS2 type of an event envelope.
pub const EVENT_TYPE: &str = "Event";
