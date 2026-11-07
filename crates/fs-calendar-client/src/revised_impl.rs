//! `RevisedCalendar` over the socket.
//!
//! Pattern: Adapter — `get` with the revision of the answer handed on as it came.

use async_trait::async_trait;
use fs_calendar_core::Result;
use fs_store::ObjectId;

use crate::RpcCalendar;
use crate::calls::events;
use crate::revised::{Revised, RevisedCalendar};

#[async_trait]
impl RevisedCalendar for RpcCalendar {
    async fn get_revised(&self, id: ObjectId) -> Result<Option<Revised>> {
        self.call(move |r| async move { events::get(&r.items, id).await })
            .await
    }
}
