//! Create, read and delete of single calendar objects.
//!
//! Pattern: Repository — the CRUD side of the calendar; every change goes through the store,
//! which logs it and makes it undoable. (`update` has its own file.)

use fs_store::{Blob, ObjectId, Query, StoredObject};

use super::consts::{CONTEXT, EVENT_TYPE};
use super::handle::CalendarRepo;
use crate::error::{Error, Result};
use crate::model::Event;
use crate::now_utc;
use crate::time::tzdb_version;

impl CalendarRepo {
    /// Validate and store a new event. Refused if its `uid` already exists.
    pub async fn create(&self, event: Event) -> Result<StoredObject> {
        self.create_inner(event, None).await
    }

    pub(super) async fn create_inner(
        &self,
        mut obj: Event,
        original: Option<Blob>,
    ) -> Result<StoredObject> {
        let now = now_utc();
        obj.normalise();
        obj.created.get_or_insert_with(|| now.clone());
        obj.updated.get_or_insert(now);
        if self.get_by_uid(&obj.uid).await?.is_some() {
            return Err(Error::Conflict(format!(
                "an object with uid {} exists",
                obj.uid
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

    /// The stored object, if it exists and is an event.
    pub async fn get(&self, id: ObjectId) -> Result<Option<StoredObject>> {
        Ok(self
            .objects
            .get(id)
            .await?
            .filter(|o| o.envelope.object_type == EVENT_TYPE))
    }

    /// The event, if `id` is an event.
    pub async fn get_event(&self, id: ObjectId) -> Result<Option<Event>> {
        Ok(self
            .get(id)
            .await?
            .map(|o| Event::from_value(o.content))
            .transpose()?)
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
