//! What the server's answer for a window does not say.
//!
//! Pattern: Decorator of the window's answer — the contract's `occurrences` carries instances and
//! nothing else: an event or an occurrence the server could not expand is left out, and a list it cut
//! is sent short, each only said on the server's own stderr (`DESIGN.md`, open points). So the answer
//! is checked against the stored events themselves: every event whose stored span reaches into the
//! days is expanded here with the same engine, and where that fails the event is named as damaged,
//! where it gives another number of instances than the server did the event's own expansion is taken.
//! Where both agree the server's instances stay. Interim, until the contract says it (a field for what
//! a window could not answer). If the list cannot be had (an error, the deadline of a call), the server's
//! answer stands and the lack of the check is said above the calendar. It costs one reading of the list of events and one expansion of the
//! events in view per reading of the days.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use fs_objects::{ObjectId, StoredObject};

use super::adapter::{CalendarAdapter, reason_text};
use super::found::{Found, damaged};
use super::marks::is_marked;

/// Does the stored span of `object` reach into `[from, to)`? An object that says nothing of its span
/// is taken as overlapping, since asking costs little.
fn overlaps(object: &StoredObject, from: DateTime<Utc>, to: DateTime<Utc>) -> bool {
    let envelope = &object.envelope;
    envelope.start_time.is_none_or(|start| start < to)
        && envelope.end_time.is_none_or(|end| end >= from)
}

impl CalendarAdapter {
    /// Make the instances of `found` whole: see the module head.
    pub(super) fn verify(
        &self,
        found: &mut Found,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<(), String> {
        let all = self
            .runtime
            .block_on(self.calendar.list_events())
            .map_err(|e| reason_text(&e))?;
        let mut counts: HashMap<ObjectId, usize> = HashMap::new();
        for instance in &found.events {
            *counts.entry(instance.object).or_default() += 1;
        }
        for object in &all {
            let id = object.envelope.id;
            let answered = counts.get(&id).copied();
            if answered.is_none() && !overlaps(object, from, to) {
                continue;
            }
            match self.expand_local(object, from, to) {
                Ok(own) if answered.unwrap_or(0) == own.len() => {}
                Ok(own) => {
                    found.events.retain(|instance| instance.object != id);
                    found.events.extend(own);
                }
                Err(why) => {
                    found.events.retain(|instance| instance.object != id);
                    found.problems.push(damaged(
                        id,
                        &object.envelope.name,
                        &why,
                        is_marked(object),
                    ));
                }
            }
        }
        Ok(())
    }
}
