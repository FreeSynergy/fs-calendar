//! Replacing an object, with `sequence` handling and optional log entries.
//!
//! Pattern: Template method — read the old object, check identity, bump `sequence` when a
//! scheduling-relevant field changed, rebuild, write with `put_and_log` so extra log entries
//! (invitations) share the transaction.

use fs_store::{LogEntry, ObjectId, StoredObject};

use super::handle::CalendarRepo;
use crate::clock::now_utc;
use crate::error::{Error, Result};
use crate::model::Event;

impl CalendarRepo {
    /// Replace the event. The `uid` cannot change; `updated` is set to now; `sequence` is
    /// incremented when a scheduling-relevant field changed (start, zone, duration, rule, status).
    pub async fn update(&self, id: ObjectId, new: Event) -> Result<StoredObject> {
        self.update_with(id, new, vec![]).await
    }

    /// [`CalendarRepo::update`] plus log entries written in the **same transaction**
    /// (`put_and_log`).
    pub(crate) async fn update_with(
        &self,
        id: ObjectId,
        mut new: Event,
        extra: Vec<LogEntry>,
    ) -> Result<StoredObject> {
        let old_stored = self.get(id).await?.ok_or(Error::NotFound(id))?;
        let old = Event::from_value(old_stored.content.clone())?;
        if old.uid != new.uid {
            return Err(Error::Conflict("the uid of an event cannot change".into()));
        }
        bump_sequence(&old, &mut new);
        let mut o = self.build(&new, Some(&old_stored.envelope), None)?;
        // keep non-derived index fields of the original import
        if let Some(v) = old_stored.index.get("tzdb_version") {
            o.index.insert("tzdb_version".into(), v.clone());
        }
        self.objects.put_and_log(o, &self.actor, extra).await?;
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
