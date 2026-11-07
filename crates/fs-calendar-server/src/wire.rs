//! Events, instances and histories on the wire.
//!
//! Pattern: Adapter — `StoredObject`, `Instance`, `InvitationEntry` and ids between Rust and
//! `events.capnp`. The JSCalendar JSON stays authoritative for the content; the envelope is the
//! routing summary.

use chrono::{DateTime, Utc};
use fs_calendar_core::model::{Bounds, parse_bounded};
use fs_calendar_core::{Event, InvitationEntry, Revision};
use fs_interfaces::rpc::{
    MAX_PAGE_BYTES, check_answer, check_intake, from_millis, read_object_id, to_millis,
    write_envelope,
};
use fs_interfaces::{common_capnp, events_capnp};
use fs_store::{ObjectId, StoredObject};

use crate::errors::{invalid, invalid_event, invalid_issue, rpc_error};

pub(crate) fn count(n: usize) -> capnp::Result<u32> {
    u32::try_from(n).map_err(|_| capnp::Error::failed("list too long".into()))
}

/// Write one event.
pub(crate) fn write_event(mut out: events_capnp::event::Builder<'_>, object: &StoredObject) {
    write_envelope(out.reborrow().init_envelope(), &object.envelope);
    let text = object.content.to_string();
    out.set_revision(Revision::of_text(object.envelope.updated, &text).as_u64());
    out.set_jscalendar(text.as_str());
}

/// Read the JSCalendar text of a request.
pub(crate) fn read_event(text: capnp::text::Reader<'_>) -> capnp::Result<Event> {
    let text = text.to_str()?;
    check_intake(text.len())?;
    // Read with the limits of an event: a text of millions of small values stops at the first past
    // them, before it is built (the request itself is the only other copy).
    let value = match parse_bounded(text, Bounds::EVENT) {
        Ok(value) => value,
        Err(fs_calendar_core::Error::Json(e)) => return Err(invalid_issue(&e.to_string())),
        Err(other) => return Err(rpc_error(other)),
    };
    Event::from_value(value).map_err(|e| invalid_event(&fs_calendar_core::Error::from(e)))
}

/// A required text of a request; empty is refused here, so it never reaches the store as an index.
pub(crate) fn required<'a>(text: capnp::text::Reader<'a>, what: &str) -> capnp::Result<&'a str> {
    let text = text.to_str()?;
    if text.is_empty() {
        return Err(invalid(format!("{what} must not be empty")));
    }
    check_intake(text.len())?;
    Ok(text)
}

/// A text of a request that may be empty.
pub(crate) fn optional(text: capnp::text::Reader<'_>) -> capnp::Result<&str> {
    let text = text.to_str()?;
    check_intake(text.len())?;
    Ok(text)
}

/// A timestamp of a request.
pub(crate) fn read_time(
    t: capnp::Result<common_capnp::timestamp::Reader<'_>>,
) -> capnp::Result<DateTime<Utc>> {
    from_millis(t?.get_unix_millis())
}

/// Write the history of an object, oldest first.
pub(crate) fn write_history(
    out: events_capnp::invitations::HistoryResults,
    entries: &[InvitationEntry],
) -> capnp::Result<()> {
    check_answer(
        entries
            .iter()
            .map(|e| {
                e.participant.len()
                    + e.activity.len()
                    + e.comment.as_ref().map_or(0, String::len)
                    + 64
            })
            .sum(),
    )?;
    let mut out = out;
    let mut list = out.get().init_items(count(entries.len())?);
    for (i, entry) in (0u32..).zip(entries) {
        let mut one = list.reborrow().get(i);
        one.set_who(entry.email.as_deref().unwrap_or(&entry.participant));
        one.set_response(entry.activity.as_str());
        one.reborrow()
            .init_at()
            .set_unix_millis(to_millis(entry.at));
        one.set_comment(entry.comment.as_deref().unwrap_or(""));
    }
    Ok(())
}

/// Read a list of ids.
pub(crate) fn read_ids(
    list: capnp::struct_list::Reader<'_, common_capnp::uuid::Owned>,
) -> capnp::Result<Vec<ObjectId>> {
    list.iter().map(read_object_id).collect()
}

/// The most a page carries; a report shares it with its ids.
pub(crate) const PAGE_BYTES: usize = MAX_PAGE_BYTES;
