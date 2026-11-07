//! Repair of events written before the index had a `page_key` (v0.4.x and older).
//!
//! Pattern: Template method (fixed flow: read in chunks, find what lacks the envelope times or the
//! `page_key`, rebuild them through the same `build` as every write, write back, report) — an event
//! without a `page_key` is skipped by every page and every window, so nothing is read before it is
//! repaired, or said. The flow is idempotent, and one event that cannot be repaired is named in the
//! report and does not stop the rest.

use fs_store::{Op, Order, Query, StoredObject};

use super::consts::{CONTEXT, EVENT_TYPE, PAGE_KEY};
use super::handle::CalendarRepo;
use crate::error::{Error, Result};
use crate::model::Event;

/// Events read per query.
const CHUNK: usize = 100;

/// What [`CalendarRepo::upgrade_index`] did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IndexReport {
    /// Events whose index and envelope times were completed.
    pub rewritten: usize,
    /// Events that could not be repaired, each as `<id>: <reason>`. They stay without a place in
    /// the order; a page and a window say so (`Error::Stored`) instead of leaving them out.
    pub failed: Vec<String>,
}

impl CalendarRepo {
    /// Complete the envelope times and the `page_key` of every event that lacks them (written before
    /// fs-store 0.2 or before v0.5.0). Run it when the repository is opened; it does nothing for events
    /// that are in order. The content and the original are kept.
    pub async fn upgrade_index(&self) -> Result<IndexReport> {
        let _one_at_a_time = self.writes.lock().await;
        let mut report = IndexReport::default();
        let mut offset = 0;
        loop {
            let q = Query::new()
                .of_type(EVENT_TYPE)
                .in_context(CONTEXT)
                .ordered(Order::PublishedAsc)
                .limit(CHUNK)
                .offset(offset);
            let chunk = self.queries.query(&q).await?;
            if chunk.is_empty() {
                return Ok(report);
            }
            offset += chunk.len();
            for object in chunk {
                if object.envelope.start_time.is_some() && object.index.contains_key(PAGE_KEY) {
                    continue;
                }
                let id = object.envelope.id;
                match self.complete(&object).await {
                    Ok(()) => report.rewritten += 1,
                    Err(e) => report.failed.push(format!("{id}: {e}")),
                }
            }
        }
    }

    /// [`CalendarRepo::upgrade_index`], and only the number of events it rewrote. Idempotent.
    pub async fn migrate_time_fields(&self) -> Result<usize> {
        Ok(self.upgrade_index().await?.rewritten)
    }

    async fn complete(&self, o: &StoredObject) -> Result<()> {
        let event = Event::from_value(o.content.clone())?;
        let mut fresh = self.build(&event, Some(&o.envelope), None, false)?;
        fresh.original.clone_from(&o.original);
        // keep non-derived index fields of the original import
        if let Some(v) = o.index.get("tzdb_version") {
            fresh.index.insert("tzdb_version".into(), v.clone());
        }
        self.objects.put(fresh, &self.actor).await?;
        Ok(())
    }

    /// Say it if an event has no place in the order: one without would be skipped by every page and
    /// every window, so a question that would leave it out fails and names the way out.
    pub(super) async fn check_places(&self) -> Result<()> {
        let all = Query::new().of_type(EVENT_TYPE).in_context(CONTEXT);
        let placed = all.clone().index_op(PAGE_KEY, Op::Ge, "");
        let (all, placed) = (
            self.queries.count(&all).await?,
            self.queries.count(&placed).await?,
        );
        if placed < all {
            return Err(Error::Stored(format!(
                "{} of {all} events have no place in the order (written before 0.5.0 or not repairable); run upgrade_index and read its report",
                all - placed
            )));
        }
        Ok(())
    }
}
