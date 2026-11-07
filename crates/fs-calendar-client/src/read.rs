//! What the server sends, as the values of the core.
//!
//! Pattern: Adapter — `Event`, `Instance` and `ImportReport` of `events.capnp` to `StoredObject`,
//! `Instance` and `ImportReport` of the core. A reply that cannot be read is `Error::Store(Backend)`
//! saying so: the request is not to blame.

use fs_calendar_core::{Error, Event, ImportReport, Instance, Occurrence, Result, Revision};
use fs_interfaces::events_capnp::{event, import_report, instance};
use fs_interfaces::rpc::{from_millis, read_envelope, read_object_id};
use fs_objects::{Error as StoreError, StoredObject};
use serde_json::Value;

use crate::revised::Revised;

/// A reply that does not read.
pub(crate) fn unreadable(e: impl std::fmt::Display) -> Error {
    Error::Store(StoreError::Backend(format!(
        "calendar server: it sent something unreadable: {e}"
    )))
}

fn wire<T>(r: capnp::Result<T>) -> Result<T> {
    r.map_err(unreadable)
}

fn text(t: capnp::Result<capnp::text::Reader<'_>>) -> Result<String> {
    wire(t.and_then(|t| Ok(t.to_string()?)))
}

fn list(l: capnp::Result<capnp::text_list::Reader<'_>>) -> Result<Vec<String>> {
    wire(l)?.iter().map(text).collect()
}

fn json(t: capnp::Result<capnp::text::Reader<'_>>) -> Result<Value> {
    serde_json::from_str(&text(t)?).map_err(unreadable)
}

/// An event: the envelope as sent, the JSCalendar text as `content`, and the revision as it came.
pub(crate) fn revised(r: event::Reader<'_>) -> Result<Revised> {
    let envelope = wire(r.get_envelope().and_then(read_envelope))?;
    Ok(Revised {
        object: StoredObject::new(
            envelope,
            fs_calendar_core::format::JSCALENDAR_MEDIA_TYPE,
            json(r.get_jscalendar())?,
        ),
        revision: Revision::from(r.get_revision()),
    })
}

/// An event without its revision.
pub(crate) fn stored(r: event::Reader<'_>) -> Result<StoredObject> {
    revised(r).map(|read| read.object)
}

/// One instance. The recurrence id is in the instance's own JSON; a single event has none, and its
/// start is the key it would have.
pub(crate) fn occurrence_of(r: instance::Reader<'_>) -> Result<Instance> {
    let event = Event::from_value(json(r.get_jscalendar())?).map_err(unreadable)?;
    let recurrence_id = event
        .recurrence_id
        .clone()
        .or_else(|| event.start.clone())
        .ok_or_else(|| unreadable("an instance with no start"))?;
    Ok(Instance {
        object: wire(r.get_object().and_then(read_object_id))?,
        occurrence: Occurrence {
            recurrence_id,
            start_utc: wire(r.get_start().and_then(|t| from_millis(t.get_unix_millis())))?,
            end_utc: wire(r.get_end().and_then(|t| from_millis(t.get_unix_millis())))?,
            event,
            overridden: r.get_overridden(),
        },
    })
}

/// The report of an import.
pub(crate) fn report(r: import_report::Reader<'_>) -> Result<ImportReport> {
    Ok(ImportReport {
        created: wire(r.get_created())?
            .iter()
            .map(|id| wire(read_object_id(id)))
            .collect::<Result<_>>()?,
        skipped: list(r.get_skipped())?,
        failed: list(r.get_failed())?,
        notes: list(r.get_notes())?,
    })
}
