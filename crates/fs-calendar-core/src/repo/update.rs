//! Replacing an object, with `sequence` handling and optional log entries.
//!
//! Pattern: Template method — read the old object, check identity (and, for compare-and-set, the
//! revision), bump `sequence` when a scheduling-relevant field changed, rebuild, write with
//! `put_and_log` so extra log entries (invitations) share the transaction, tell the feed.
//! The public writes take turns, so check and write are one step.

use fs_store::{LogEntry, ObjectId, StoredObject};

use super::handle::CalendarRepo;
use crate::error::{Error, Result};
use crate::feed::CalendarChange;
use crate::model::Event;
use crate::now_utc;
use crate::revision::Revision;
use crate::uid::normalise_uid;

impl CalendarRepo {
    /// Replace the event. The `uid` cannot change; `updated` is set to now; `sequence` is
    /// incremented when a scheduling-relevant field changed (start, zone, duration, rule, status).
    pub async fn update(&self, id: ObjectId, new: Event) -> Result<StoredObject> {
        let _one_at_a_time = self.writes.lock().await;
        self.update_with(id, new, vec![]).await
    }

    /// [`CalendarRepo::update`] only if the stored event is still at `expected`; otherwise
    /// [`Error::Conflict`] and nothing is written. Compare and write are one step.
    pub async fn update_checked(
        &self,
        id: ObjectId,
        expected: &Revision,
        new: Event,
    ) -> Result<StoredObject> {
        let _one_at_a_time = self.writes.lock().await;
        let stored = self.get(id).await?.ok_or(Error::NotFound(id))?;
        if Revision::of(&stored) != *expected {
            return Err(Error::Conflict(format!(
                "conflict: the event was changed by someone else (now at revision {})",
                Revision::of(&stored)
            )));
        }
        self.update_with(id, new, vec![]).await
    }

    /// [`CalendarRepo::update`] plus log entries written in the **same transaction**
    /// (`put_and_log`). The caller holds the turn (`writes`).
    pub(crate) async fn update_with(
        &self,
        id: ObjectId,
        mut new: Event,
        extra: Vec<LogEntry>,
    ) -> Result<StoredObject> {
        let old_stored = self.get(id).await?.ok_or(Error::NotFound(id))?;
        let old = Event::from_value(old_stored.content.clone())?;
        if normalise_uid(&old.uid) != normalise_uid(&new.uid) {
            return Err(Error::Conflict("the uid of an event cannot change".into()));
        }
        bump_sequence(&old, &mut new);
        let mut o = self.build(
            &new,
            Some(&old_stored.envelope),
            None,
            new.recurrence_rule != old.recurrence_rule,
        )?;
        // keep non-derived index fields of the original import
        if let Some(v) = old_stored.index.get("tzdb_version") {
            o.index.insert("tzdb_version".into(), v.clone());
        }
        self.objects.put_and_log(o, &self.actor, extra).await?;
        self.feed.notify(&CalendarChange::Updated(id));
        self.get(id).await?.ok_or(Error::NotFound(id))
    }
}

/// Set `updated`, keep `created`, and increment `sequence` on scheduling-relevant changes.
fn bump_sequence(old: &Event, new: &mut Event) {
    new.updated = Some(now_utc());
    new.created = new.created.take().or_else(|| old.created.clone());
    let rel = |e: &Event| {
        (
            e.start.clone(),
            e.time_zone.clone(),
            e.duration.clone(),
            e.recurrence_rule.clone(),
            e.status.clone(),
        )
    };
    if rel(old) != rel(new) && new.sequence.unwrap_or(0) <= old.sequence.unwrap_or(0) {
        new.sequence = Some(old.sequence.unwrap_or(0) + 1);
    }
}
