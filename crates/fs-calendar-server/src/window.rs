//! The answer of `Items.occurrences` on the wire: the instances, what is damaged, and whether it is cut.
//!
//! Pattern: Adapter — a core [`Window`] becomes the `items`, `damaged` and `cut` of `events.capnp`.
//! The core cuts at its bounds and names what it cut; here the instances are weighed once more, as
//! the JSON that is really written, and an answer above a message is refused as too large.

use fs_calendar_core::limits::INSTANCE_OVERHEAD_BYTES;
use fs_calendar_core::{Damaged, Instance, Window};
use fs_interfaces::events_capnp;
use fs_interfaces::rpc::{check_answer, to_millis, write_object_id};

use crate::wire::count;

/// Counts the bytes written to it and keeps none.
struct Counting(usize);

impl std::io::Write for Counting {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0 += buf.len();
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// The length of the JSON of an instance, without holding its text.
fn json_len(instance: &Instance) -> usize {
    let mut count = Counting(0);
    // A value always serialises; counting cannot fail.
    let _ = serde_json::to_writer(&mut count, &instance.occurrence.event.to_value());
    count.0
}

/// Write the instances of a window. All of them go in one answer, so an answer above a page is refused
/// before anything is written, counting one instance at a time and stopping at the first that passes
/// the page: the consumer asks for a shorter window. Texts are made one at a time, never all at once.
fn write_instances(
    out: &mut events_capnp::items::OccurrencesResults,
    found: &[Instance],
) -> capnp::Result<()> {
    let mut total = 0usize;
    for instance in found {
        total = total.saturating_add(json_len(instance) + INSTANCE_OVERHEAD_BYTES);
        check_answer(total)?;
    }
    let mut list = out.get().init_items(count(found.len())?);
    for (i, instance) in (0u32..).zip(found) {
        let mut one = list.reborrow().get(i);
        write_object_id(one.reborrow().init_object(), instance.object);
        one.reborrow()
            .init_start()
            .set_unix_millis(to_millis(instance.occurrence.start_utc));
        one.reborrow()
            .init_end()
            .set_unix_millis(to_millis(instance.occurrence.end_utc));
        one.reborrow()
            .set_jscalendar(instance.occurrence.event.to_value().to_string().as_str());
        one.set_overridden(instance.occurrence.overridden);
    }
    Ok(())
}

/// Write what the window could not answer for. The `key` is empty for a whole event.
fn write_damaged(
    out: &mut events_capnp::items::OccurrencesResults,
    damaged: &[Damaged],
) -> capnp::Result<()> {
    let mut list = out.get().init_damaged(count(damaged.len())?);
    for (i, d) in (0u32..).zip(damaged) {
        let mut one = list.reborrow().get(i);
        write_object_id(one.reborrow().init_id(), d.object);
        one.reborrow().set_reason(d.reason.as_str());
        one.set_key(d.key.as_deref().unwrap_or(""));
    }
    Ok(())
}

/// Write the answer of a window: its instances, what is damaged, and `cut` if the answer is
/// incomplete because a bound was reached. The answer is then shorter, not refused.
pub(crate) fn write_window(
    mut out: events_capnp::items::OccurrencesResults,
    window: &Window,
) -> capnp::Result<()> {
    say_damage(&window.damaged);
    write_instances(&mut out, &window.instances)?;
    write_damaged(&mut out, &window.damaged)?;
    out.get().set_cut(window.damaged.iter().any(|d| d.cut));
    Ok(())
}

/// Say on stderr what a window could not answer for, by the id of the event.
pub(crate) fn say_damage(damaged: &[Damaged]) {
    for d in damaged {
        eprintln!(
            "fs-calendar-server: occurrences: event {}{}: {}",
            d.object,
            d.key.as_ref().map_or(String::new(), |k| format!(" at {k}")),
            d.reason
        );
    }
}
