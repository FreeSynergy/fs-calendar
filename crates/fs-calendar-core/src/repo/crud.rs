//! Create, read and delete of single calendar objects.
//!
//! Pattern: Repository — the CRUD side of the calendar; every change goes through the store,
//! which logs it and makes it undoable. (`update` has its own file.)

use fs_store::{Blob, ObjectId, Query, StoredObject};

use super::consts::{CONTEXT, EVENT_TYPE, TASK_TYPE};
use super::handle::CalendarRepo;
use crate::clock::now_utc;
use crate::error::{Error, Result};
use crate::model::{CalendarObject, Event, Task};
use crate::time::tzdb_version;

impl CalendarRepo {
    /// Validate and store a new event or task. Refused if its `uid` already exists.
    pub async fn create(&self, obj: impl Into<CalendarObject>) -> Result<StoredObject> {
        self.create_inner(obj.into(), None).await
    }

    pub(super) async fn create_inner(
        &self,
        mut obj: CalendarObject,
        original: Option<Blob>,
    ) -> Result<StoredObject> {
        let now = now_utc();
        match &mut obj {
            CalendarObject::Event(e) => {
                e.normalise();
                e.created.get_or_insert_with(|| now.clone());
                e.updated.get_or_insert(now);
            }
            CalendarObject::Task(t) => {
                t.normalise();
                t.created.get_or_insert_with(|| now.clone());
                t.updated.get_or_insert(now);
            }
        }
        if self.get_by_uid(obj.uid()).await?.is_some() {
            return Err(Error::Conflict(format!(
                "an object with uid {} exists",
                obj.uid()
            )));
        }
        let mut o = self.build(&obj, None, original.clone())?;
        if original.is_some() {
            o.index
                .entry("tzdb_version".into())
                .or_default()
                .push(tzdb_version().into());
        }
        let id = o.envelope.id;
        self.objects.put(o, &self.actor).await?;
        self.get(id).await?.ok_or(Error::NotFound(id))
    }

    /// The stored object, if it exists and is an event or task.
    pub async fn get(&self, id: ObjectId) -> Result<Option<StoredObject>> {
        Ok(self
            .objects
            .get(id)
            .await?
            .filter(|o| matches!(o.envelope.object_type.as_str(), EVENT_TYPE | TASK_TYPE)))
    }

    /// The typed object (event or task), if it exists.
    pub async fn get_object(&self, id: ObjectId) -> Result<Option<CalendarObject>> {
        self.get(id)
            .await?
            .map(|o| CalendarObject::from_value(o.content))
            .transpose()
    }

    /// The event, if `id` is an event.
    pub async fn get_event(&self, id: ObjectId) -> Result<Option<Event>> {
        Ok(match self.get_object(id).await? {
            Some(CalendarObject::Event(e)) => Some(e),
            _ => None,
        })
    }

    /// The task, if `id` is a task.
    pub async fn get_task(&self, id: ObjectId) -> Result<Option<Task>> {
        Ok(match self.get_object(id).await? {
            Some(CalendarObject::Task(t)) => Some(t),
            _ => None,
        })
    }

    /// Lookup by the object's `uid`.
    pub async fn get_by_uid(&self, uid: &str) -> Result<Option<StoredObject>> {
        let q = Query::new()
            .in_context(CONTEXT)
            .with_index("uid", uid)
            .limit(1);
        Ok(self.queries.query(&q).await?.into_iter().next())
    }

    /// Delete an object (undoable through the store).
    pub async fn delete(&self, id: ObjectId) -> Result<()> {
        self.get(id).await?.ok_or(Error::NotFound(id))?;
        self.objects.delete(id, &self.actor).await?;
        Ok(())
    }
}
