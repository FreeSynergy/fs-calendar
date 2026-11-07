//! Adapter from the `Store` facade to the three narrow traits the calendar needs.
//!
//! Pattern: Adapter — `CalendarRepo::new` still takes `Arc<dyn Store>`, but the repository itself
//! only sees `dyn ObjectRepository`, `dyn ObjectQuery` and `dyn ActivityLog` (interface
//! segregation, ADR 0043).

use std::sync::Arc;

use async_trait::async_trait;
use fs_objects::traits::{ActivityLog, ObjectQuery, ObjectRepository};
use fs_objects::{
    Activity, ActivityFilter, ActivityKind, LogEntry, ObjectId, Query, Result, Store, StoreId,
    StoreInfo, StoredObject,
};

/// Presents an `Arc<dyn Store>` as the narrow traits.
pub(super) struct StoreAdapter(pub(super) Arc<dyn Store>);

#[async_trait]
impl ObjectRepository for StoreAdapter {
    async fn put(&self, object: StoredObject, actor: &str) -> Result<Activity> {
        Store::put(&*self.0, object, actor).await
    }

    async fn put_and_log(
        &self,
        object: StoredObject,
        actor: &str,
        extra: Vec<LogEntry>,
    ) -> Result<Vec<Activity>> {
        Store::put_and_log(&*self.0, object, actor, extra).await
    }

    async fn get(&self, id: ObjectId) -> Result<Option<StoredObject>> {
        Store::get(&*self.0, id).await
    }

    async fn delete(&self, id: ObjectId, actor: &str) -> Result<Activity> {
        Store::delete(&*self.0, id, actor).await
    }
}

#[async_trait]
impl ObjectQuery for StoreAdapter {
    async fn query(&self, query: &Query) -> Result<Vec<StoredObject>> {
        Store::query(&*self.0, query).await
    }

    async fn count(&self, query: &Query) -> Result<usize> {
        Store::count(&*self.0, query).await
    }
}

#[async_trait]
impl ActivityLog for StoreAdapter {
    async fn log(
        &self,
        kind: ActivityKind,
        object: ObjectId,
        actor: &str,
        payload: serde_json::Value,
    ) -> Result<Activity> {
        Store::log(&*self.0, kind, object, actor, payload).await
    }

    async fn activities(&self, filter: &ActivityFilter) -> Result<Vec<Activity>> {
        Store::activities(&*self.0, filter).await
    }
}

impl StoreInfo for StoreAdapter {
    fn identity(&self) -> StoreId {
        Store::identity(&*self.0)
    }

    fn is_encrypted(&self) -> bool {
        Store::is_encrypted(&*self.0)
    }
}
