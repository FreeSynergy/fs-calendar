//! A page of events on the wire.
//!
//! Pattern: Adapter — an [`EventPage`] from the provider becomes an `EventPage` of `events.capnp`,
//! cut by weight so the answer always fits a message, with the cursor of the event the page ends at.
//! The page was cut in the provider's store; here only its weight is looked at. The cursor is the
//! id of the last event (a place in the sort key), not a count, so an event added or removed between
//! two pages moves no boundary.

use fs_calendar_core::{Calendar, EventPage};
use fs_interfaces::events_capnp;
use fs_interfaces::rpc::{
    MAX_ITEM_BYTES, MAX_PAGE_BYTES, Problem, TOO_LARGE, envelope_weight, invalid_request,
};
use fs_store::{ObjectId, StoredObject};

use crate::errors::rpc_error;
use crate::wire::{count, write_event};

/// The first byte of a cursor: 1 was a position in the whole list (`fs-interfaces` `Page`), 2 is an
/// id. Form 1 is not read any more: a position moves when an event is added or removed.
const FORM: u8 = 2;

/// The cursor that continues after `id`.
fn encode(id: ObjectId) -> Vec<u8> {
    [&[FORM][..], id.to_string().as_bytes()].concat()
}

/// The id a cursor names; empty = the start.
fn decode(cursor: &[u8]) -> capnp::Result<Option<ObjectId>> {
    let Some((&form, id)) = cursor.split_first() else {
        return Ok(None);
    };
    let invalid = || invalid_request("invalid cursor");
    let id = std::str::from_utf8(id).map_err(|_| invalid())?;
    if form != FORM {
        return Err(invalid());
    }
    id.parse().map(Some).map_err(|_| invalid())
}

fn weight(o: &StoredObject) -> usize {
    o.content.to_string().len() + envelope_weight(&o.envelope)
}

/// How many of the items go into this answer: all, or fewer if they weigh more than a message
/// carries. An item that cannot be sent even alone is a `too-large` problem whose `next` is the
/// cursor past it (none if it was the last), so what is behind it stays readable.
fn fitting(page: &EventPage) -> capnp::Result<usize> {
    let (mut taken, mut bytes) = (0usize, 0usize);
    for (i, item) in page.items.iter().enumerate() {
        let heavy = weight(item);
        bytes = bytes.saturating_add(heavy);
        let full = bytes > MAX_PAGE_BYTES;
        if full && taken > 0 {
            break;
        }
        if heavy > MAX_ITEM_BYTES {
            let later = i + 1 < page.items.len() || page.more;
            let detail = format!("one item of {heavy} bytes, at most {MAX_ITEM_BYTES} fit");
            // With a cursor no extensions: the numbers are in the detail anyway.
            let problem = if later {
                Problem::new(TOO_LARGE, "Too large", detail, 413)
            } else {
                Problem::too_large(detail, heavy as u64, MAX_ITEM_BYTES as u64)
            };
            return Err(problem
                .with_next(later.then(|| encode(item.envelope.id)))
                .into_error());
        }
        taken += 1;
        if full {
            break;
        }
    }
    Ok(taken)
}

/// The page of events that `cursor` asks for.
pub(crate) async fn read_page(
    calendar: &dyn Calendar,
    cursor: &[u8],
    size: usize,
) -> capnp::Result<EventPage> {
    let after = decode(cursor)?;
    calendar.list_page(after, size).await.map_err(rpc_error)
}

/// Write the page.
pub(crate) fn write_page(
    mut out: events_capnp::event_page::Builder<'_>,
    page: &EventPage,
) -> capnp::Result<()> {
    let taken = fitting(page)?;
    let mut items = out.reborrow().init_items(count(taken)?);
    for (i, object) in (0u32..).zip(&page.items[..taken]) {
        write_event(items.reborrow().get(i), object);
    }
    let next = match page.items[..taken].last() {
        Some(last) if taken < page.items.len() || page.more => encode(last.envelope.id),
        _ => Vec::new(),
    };
    out.init_next().set_opaque(&next);
    Ok(())
}
