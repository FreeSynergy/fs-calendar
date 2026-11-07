//! A page of events, cut in the store after the id of the last one read.
//!
//! Pattern: Iterator (external, resumable) over the Repository — the page after an id is asked of the
//! store as one query with a limit, never as the whole list that is cut afterwards. The order is
//! the store's `order_by_index` of `page_key` (the event's id as lower case text, unique, so the
//! store sorts and compares it alike), and the condition is `page_key > id`: no event falls between
//! two pages and none comes twice, whatever is added or removed in between.

use fs_objects::{ObjectId, Op, Query, StoredObject};

use super::consts::{CONTEXT, EVENT_TYPE, PAGE_KEY};
use super::handle::CalendarRepo;
use crate::error::Result;

/// One page of events in the order of their ids.
#[derive(Clone, Debug, Default)]
pub struct EventPage {
    /// At most the size asked for.
    pub items: Vec<StoredObject>,
    /// There is at least one more event after the last of `items`.
    pub more: bool,
}

impl EventPage {
    /// The page after `after` of events in any order: for a provider with no store to cut in.
    pub fn of_all(mut all: Vec<StoredObject>, after: Option<ObjectId>, size: usize) -> Self {
        let after = after.map(|id| id.to_string());
        all.retain(|o| {
            after
                .as_ref()
                .is_none_or(|a| o.envelope.id.to_string() > *a)
        });
        all.sort_by_key(|o| o.envelope.id.to_string());
        let more = all.len() > size;
        all.truncate(size);
        Self { items: all, more }
    }
}

impl CalendarRepo {
    /// At most `size` events after the event `after` (`None` = from the start), in the order of
    /// their ids. The store reads these rows and one more, not the events before them.
    pub async fn list_page(&self, after: Option<ObjectId>, size: usize) -> Result<EventPage> {
        if after.is_none() {
            self.check_places().await?;
        }
        let mut query = Query::new()
            .of_type(EVENT_TYPE)
            .in_context(CONTEXT)
            .order_by_index(PAGE_KEY);
        if let Some(id) = after {
            query = query.index_op(PAGE_KEY, Op::Gt, id.to_string());
        }
        // One more than asked: that is how a page knows there is a next one.
        let mut items = self
            .queries
            .query(&query.limit(size.saturating_add(1)))
            .await?;
        let more = items.len() > size;
        items.truncate(size);
        Ok(EventPage { items, more })
    }
}
