//! Replacing an object, with `sequence` handling and optional log entries.
//!
//! Pattern: Template method — read the old object, check identity, bump `sequence` when a
//! scheduling-relevant field changed, rebuild, write with `put_and_log` so extra log entries
//! (invitations) share the transaction.

use fs_store::{LogEntry, ObjectId, StoredObject};

use super::handle::CalendarRepo;
use crate::clock::now_utc;
use crate::error::{Error, Result};
use crate::model::{CalendarObject, Event};

impl CalendarRepo {
    /// Replace the object. `uid` and type cannot change; `updated` is set to now; `sequence` is
    /// incremented when a scheduling-relevant field changed (start, zone, duration, rule,
    /// status, due).
    pub async fn update(
        &self,
        id: ObjectId,
        new: impl Into<CalendarObject>,
    ) -> Result<StoredObject> {
        self.update_with(id, new.into(), vec![]).await
    }

    /// [`CalendarRepo::update`] plus log entries written in the **same transaction**
    /// (`put_and_log`).
    pub(crate) async fn update_with(
        &self,
        id: ObjectId,
        mut new: CalendarObject,
        extra: Vec<LogEntry>,
    ) -> Result<StoredObject> {
        let old_stored = self.get(id).await?.ok_or(Error::NotFound(id))?;
        let old = CalendarObject::from_value(old_stored.content.clone())?;
        if old.uid() != new.uid() || old.type_name() != new.type_name() {
            return Err(Error::Conflict(
                "uid and type of an object cannot change".into(),
            ));
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
fn bump_sequence(old: &CalendarObject, new: &mut CalendarObject) {
    let now = now_utc();
    match (old, new) {
        (CalendarObject::Event(o), CalendarObject::Event(n)) => {
            n.updated = Some(now);
            n.created = n.created.take().or_else(|| o.created.clone());
            let rel = |e: &Event| {
                (
                    e.start.clone(),
                    e.time_zone.clone(),
                    e.duration.clone(),
                    e.recurrence_rule.clone(),
                    e.status.clone(),
                )
            };
            if rel(o) != rel(n) && n.sequence.unwrap_or(0) <= o.sequence.unwrap_or(0) {
                n.sequence = Some(o.sequence.unwrap_or(0) + 1);
            }
        }
        (CalendarObject::Task(o), CalendarObject::Task(n)) => {
            n.updated = Some(now);
            n.created = n.created.take().or_else(|| o.created.clone());
            let changed = (&o.start, &o.due, &o.time_zone) != (&n.start, &n.due, &n.time_zone);
            if changed && n.sequence.unwrap_or(0) <= o.sequence.unwrap_or(0) {
                n.sequence = Some(o.sequence.unwrap_or(0) + 1);
            }
        }
        _ => unreachable!("type checked by the caller"),
    }
}
