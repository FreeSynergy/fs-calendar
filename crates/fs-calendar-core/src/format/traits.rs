//! The [`Importer`] and [`Exporter`] contracts.
//!
//! Pattern: Strategy + Template method — the repository owns the fixed import flow (split, skip
//! known uids, parse, normalise, store, report); a format only says how to split a text into
//! parts and how to parse one part. Every foreign format is an adapter implementing these traits.

use fs_objects::Blob;

use crate::error::Result;
use crate::model::Event;

/// One independently importable piece of a source text (for iCalendar: all components of one UID).
#[derive(Clone, Debug)]
pub struct Part {
    /// The `uid`, when it is known without parsing (lets the flow skip known objects early).
    pub uid: Option<String>,
    /// The part's own text.
    pub text: String,
}

/// A source text cut into parts.
#[derive(Clone, Debug, Default)]
pub struct Split {
    /// The parts, in source order.
    pub parts: Vec<Part>,
    /// Notes about changes made while splitting (e.g. the 1.0 -> 2.0 upgrade).
    pub notes: Vec<String>,
}

/// A task an importer found in a file that is not ours (ADR 0050): not read, only handed on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskPart {
    /// Its `uid`, as written (empty if it has none).
    pub uid: String,
    /// The JSCalendar 2.0 `Task` as JSON text.
    pub jscalendar: String,
}

/// One part converted to calendar objects.
#[derive(Clone, Debug)]
pub struct Parsed {
    /// The events found in the part (usually one; an event with overrides may give several).
    pub objects: Vec<Event>,
    /// Tasks found in the part. They are not ours (ADR 0050): the flow hands them to the task program.
    pub tasks: Vec<TaskPart>,
    /// The source text to keep as original (ADR 0030), if the format has one.
    pub original: Option<Blob>,
    /// Notes about changes made while parsing.
    pub notes: Vec<String>,
}

/// Reads calendar objects from the text of one foreign format.
pub trait Importer: Send + Sync {
    /// Media type this importer reads (e.g. `text/calendar`).
    fn media_type(&self) -> &'static str;

    /// Cut `text` into parts. `Err` means the text is unusable as a whole.
    fn split(&self, text: &str) -> Result<Split>;

    /// Convert one part. `Err` is reported for that part only, never silently dropped.
    fn parse(&self, part: &Part) -> Result<Parsed>;
}

/// Writes calendar objects as text of one foreign format.
pub trait Exporter: Send + Sync {
    /// Media type this exporter produces.
    fn output_media_type(&self) -> &'static str;

    /// The objects as one text of this format.
    fn export(&self, objects: &[Event]) -> Result<String>;
}
