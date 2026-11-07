//! Events, instances and histories on the wire.
//!
//! Pattern: Adapter — `StoredObject`, `Instance`, `InvitationEntry` and ids between Rust and
//! `events.capnp`. The JSCalendar JSON stays authoritative for the content; the envelope is the
//! routing summary.

use chrono::{DateTime, Utc};
use fs_calendar_core::{Event, Instance, InvitationEntry};
use fs_interfaces::rpc::{
    DEFAULT_PAGE_SIZE, MAX_PAGE_BYTES, Page, check_answer, check_intake, envelope_weight,
    from_millis, read_object_id, to_millis, write_envelope, write_object_id,
};
use fs_interfaces::{common_capnp, events_capnp};
use fs_store::{ObjectId, StoredObject};

use crate::errors::invalid;

pub(crate) fn count(n: usize) -> capnp::Result<u32> {
    u32::try_from(n).map_err(|_| capnp::Error::failed("list too long".into()))
}

/// Write one event.
pub(crate) fn write_event(mut out: events_capnp::event::Builder<'_>, object: &StoredObject) {
    write_envelope(out.reborrow().init_envelope(), &object.envelope);
    out.set_jscalendar(object.content.to_string().as_str());
}

/// Read the JSCalendar text of a request.
pub(crate) fn read_event(text: capnp::text::Reader<'_>) -> capnp::Result<Event> {
    let text = text.to_str()?;
    check_intake(text.len())?;
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|e| invalid(e.to_string()))?;
    Event::from_value(value).map_err(|e| invalid(e.to_string()))
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

/// Cut the page at `cursor` and write it.
pub(crate) fn write_page(
    mut out: events_capnp::event_page::Builder<'_>,
    all: Vec<StoredObject>,
    cursor: capnp::Result<common_capnp::cursor::Reader<'_>>,
) -> capnp::Result<()> {
    let opaque = cursor?.get_opaque()?;
    let page = Page::of_weighted(all, opaque, DEFAULT_PAGE_SIZE, |o: &StoredObject| {
        o.content.to_string().len() + envelope_weight(&o.envelope)
    })?;
    let mut items = out.reborrow().init_items(count(page.items.len())?);
    for (i, object) in (0u32..).zip(&page.items) {
        write_event(items.reborrow().get(i), object);
    }
    out.init_next().set_opaque(&page.next);
    Ok(())
}

/// What one instance weighs beyond its text: ids, times, structure.
const INSTANCE_BYTES: usize = 96;

/// Write the instances of a window. All of them go in one answer, so an answer above a page is refused
/// before anything is written: the consumer asks for a shorter window.
pub(crate) fn write_instances(
    out: events_capnp::items::OccurrencesResults,
    found: &[Instance],
) -> capnp::Result<()> {
    let texts: Vec<String> = found
        .iter()
        .map(|i| i.occurrence.event.to_value().to_string())
        .collect();
    check_answer(
        texts
            .iter()
            .map(|t| t.len() + INSTANCE_BYTES)
            .sum::<usize>(),
    )?;
    let mut out = out;
    let mut list = out.get().init_items(count(found.len())?);
    for (i, (instance, text)) in (0u32..).zip(found.iter().zip(&texts)) {
        let mut one = list.reborrow().get(i);
        write_object_id(one.reborrow().init_object(), instance.object);
        one.reborrow()
            .init_start()
            .set_unix_millis(to_millis(instance.occurrence.start_utc));
        one.reborrow()
            .init_end()
            .set_unix_millis(to_millis(instance.occurrence.end_utc));
        one.reborrow().set_jscalendar(text.as_str());
        one.set_overridden(instance.occurrence.overridden);
    }
    Ok(())
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
