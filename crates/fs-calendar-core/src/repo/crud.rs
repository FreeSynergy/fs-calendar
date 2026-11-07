//! Create, read and delete of single calendar objects.
//!
//! Pattern: Repository — the CRUD side of the calendar; every change goes through the store,
//! which logs it and makes it undoable, and through the feed, which tells the subscribers. Public
//! writes take turns (`writes`); the `_inner` flows do not, so they compose without waiting on
//! themselves. (`update` has its own file.)

use fs_store::{Blob, Cond, ObjectId, Op, Query, StoredObject};

use super::consts::{CONTEXT, EVENT_TYPE};
use super::handle::CalendarRepo;
use crate::error::{Error, Result};
use crate::feed::CalendarChange;
use crate::model::Event;
use crate::now_utc;
use crate::time::tzdb_version;
use crate::uid::normalise_uid;

impl CalendarRepo {
    /// Validate and store a new event. Refused if its `uid` already exists, in any spelling.
    pub async fn create(&self, event: Event) -> Result<StoredObject> {
        let _one_at_a_time = self.writes.lock().await;
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
                fs_jscalendar::quoted(&obj.uid)
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
        // A uid that is a UUID becomes the object id, so it can name an object that already exists.
        if self.objects.get(id).await?.is_some() {
            return Err(Error::Conflict(format!("an object with id {id} exists")));
        }
        self.objects.put(o, &self.actor).await?;
        self.feed.notify(&CalendarChange::Created(id));
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

    /// Lookup by the object's `uid`, in any spelling of the same identity (`normalise_uid`).
    pub async fn get_by_uid(&self, uid: &str) -> Result<Option<StoredObject>> {
        let key = normalise_uid(uid);
        let same = |o: &StoredObject| {
            o.content
                .get("uid")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|stored| normalise_uid(stored) == key)
        };
        let is = |index: &str, value: &str| Cond::Index {
            key: index.into(),
            op: Op::Eq,
            value: value.into(),
        };
        // `uid` finds what was stored before `uid_key` existed.
        let q = Query::new()
            .of_type(EVENT_TYPE)
            .in_context(CONTEXT)
            .or(is("uid_key", &key))
            .or(is("uid", uid));
        Ok(self.queries.query(&q).await?.into_iter().find(same))
    }

    /// Delete an object (undoable through the store).
    pub async fn delete(&self, id: ObjectId) -> Result<()> {
        let _one_at_a_time = self.writes.lock().await;
        self.get(id).await?.ok_or(Error::NotFound(id))?;
        self.objects.delete(id, &self.actor).await?;
        self.feed.notify(&CalendarChange::Deleted(id));
        Ok(())
    }
}
