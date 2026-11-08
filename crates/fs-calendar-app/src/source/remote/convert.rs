//! What the wire holds, as the core's types.
//!
//! Pattern: Adapter (pure functions).

use fs_calendar_core::{Damaged, Error, Event, Instance, Occurrence, Result};
use fs_interfaces::events_capnp::{damaged, event, instance};
use fs_interfaces::rpc::{from_millis, read_envelope, read_object_id};
use fs_objects::StoredObject;

fn wire(e: &capnp::Error) -> Error {
    Error::Stored(format!("the calendar server: {e}"))
}

fn text(t: capnp::Result<capnp::text::Reader<'_>>) -> Result<String> {
    t.map_err(|e| wire(&e))?
        .to_string()
        .map_err(|e| Error::Stored(e.to_string()))
}

/// An event of the wire as a stored object: the envelope and the JSCalendar content.
pub(super) fn stored(e: event::Reader<'_>) -> Result<StoredObject> {
    let envelope = read_envelope(e.get_envelope().map_err(|e| wire(&e))?).map_err(|e| wire(&e))?;
    let content = serde_json::from_str(&text(e.get_jscalendar())?)?;
    Ok(StoredObject::new(
        envelope,
        "application/jscalendar+json",
        content,
    ))
}

/// One instance of the wire.
pub(super) fn instance_of(i: instance::Reader<'_>) -> Result<Instance> {
    let object = read_object_id(i.get_object().map_err(|e| wire(&e))?).map_err(|e| wire(&e))?;
    let start = i.get_start().map_err(|e| wire(&e))?.get_unix_millis();
    let end = i.get_end().map_err(|e| wire(&e))?.get_unix_millis();
    let event = Event::from_value(serde_json::from_str(&text(i.get_jscalendar())?)?)?;
    Ok(Instance {
        object,
        occurrence: Occurrence {
            recurrence_id: event.recurrence_id.clone().unwrap_or_default(),
            start_utc: from_millis(start).map_err(|e| wire(&e))?,
            end_utc: from_millis(end).map_err(|e| wire(&e))?,
            event,
            overridden: i.get_overridden(),
        },
    })
}

/// One damaged part of the wire.
pub(super) fn damaged_of(d: damaged::Reader<'_>) -> Result<Damaged> {
    let reason = text(d.get_reason())?;
    let key = text(d.get_key())?;
    Ok(Damaged {
        object: read_object_id(d.get_id().map_err(|e| wire(&e))?).map_err(|e| wire(&e))?,
        cut: reason.starts_with("cut: "),
        key: (!key.is_empty()).then_some(key),
        reason,
    })
}

/// A failed call as the core's error.
pub(super) fn failed(e: &capnp::Error) -> Error {
    wire(e)
}
