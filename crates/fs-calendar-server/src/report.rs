//! The report of an import on the wire.
//!
//! Pattern: Adapter — `ImportReport` between Rust and `events.capnp`, cut to what one answer holds:
//! `created` stays whole (the consumer has no other way to learn the ids), the texts share what is left.

use std::borrow::Cow;

use fs_calendar_core::ImportReport;
use fs_interfaces::events_capnp;
use fs_interfaces::rpc::{capped, write_object_id};

use crate::wire::{PAGE_BYTES, count};

/// What one id of a report weighs, and one text beyond its own bytes.
const ID_BYTES: usize = 24;
const TEXT_BYTES: usize = 16;
/// Room for the last line of a list that was cut.
const MORE_BYTES: usize = 64;

/// The texts of `items` that fit `budget`, each cut to what an answer carries, ending with
/// `... N more` if not all do. Takes what it used from `budget`.
fn fitting<'a>(items: &'a [String], budget: &mut usize) -> Vec<Cow<'a, str>> {
    let mut kept: Vec<Cow<'a, str>> = Vec::new();
    for text in items {
        let text = capped(text);
        let weight = text.len() + TEXT_BYTES;
        if weight > *budget {
            kept.push(format!("... {} more", items.len() - kept.len()).into());
            break;
        }
        *budget -= weight;
        kept.push(text.into());
    }
    kept
}

fn texts(list: capnp::text_list::Builder<'_>, items: &[Cow<'_, str>]) {
    let mut list = list;
    for (i, s) in (0u32..).zip(items) {
        list.set(i, s.as_ref());
    }
}

/// Write the report of an import. The texts share what is left of a page, what failed first.
pub(crate) fn write_report(
    mut out: events_capnp::import_report::Builder<'_>,
    report: &ImportReport,
) -> capnp::Result<()> {
    let mut created = out.reborrow().init_created(count(report.created.len())?);
    for (i, id) in (0u32..).zip(&report.created) {
        write_object_id(created.reborrow().get(i), *id);
    }
    let mut budget = PAGE_BYTES
        .saturating_sub(report.created.len().saturating_mul(ID_BYTES))
        .saturating_sub(3 * MORE_BYTES);
    let failed = fitting(&report.failed, &mut budget);
    let skipped = fitting(&report.skipped, &mut budget);
    let notes = fitting(&report.notes, &mut budget);
    texts(out.reborrow().init_failed(count(failed.len())?), &failed);
    texts(out.reborrow().init_skipped(count(skipped.len())?), &skipped);
    texts(out.init_notes(count(notes.len())?), &notes);
    Ok(())
}
