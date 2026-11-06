//! Listing and time-range queries.
//!
//! Pattern: Repository (read side) + Iterator-style expansion — the store selects candidates by
//! the envelope times and index fields; only those are expanded into instances by the
//! recurrence engine.

use chrono::{DateTime, Duration, Utc};
use fs_store::{Op, Order, Query, StoredObject, TimeField};

use super::consts::{CONTEXT, EVENT_TYPE, TASK_TYPE};
use super::handle::CalendarRepo;
use super::types::Instance;
use crate::error::Result;
use crate::model::Event;
use crate::recur::expand_with;
use crate::time::fmt_utc;

impl CalendarRepo {
    /// All events, ordered by first start (by the store).
    pub async fn list_events(&self) -> Result<Vec<StoredObject>> {
        let q = Query::new()
            .of_type(EVENT_TYPE)
            .in_context(CONTEXT)
            .ordered(Order::StartAsc);
        Ok(self.queries.query(&q).await?)
    }

    /// All tasks, ordered by due date / end (undated last), by the store.
    pub async fn list_tasks(&self) -> Result<Vec<StoredObject>> {
        let q = Query::new()
            .of_type(TASK_TYPE)
            .in_context(CONTEXT)
            .order_by_index("end_utc");
        Ok(self.queries.query(&q).await?)
    }

    /// Event instances overlapping `[from, to)`, recurring series expanded at query time,
    /// ordered by start.
    ///
    /// The store selects the candidates with a range query on the envelope times
    /// (`start_time < to AND end_time >= from`; a series carries its last end, an unbounded one
    /// the `FOREVER` sentinel), and only those are expanded.
    pub async fn occurrences(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<Instance>> {
        let q = Query::new()
            .of_type(EVENT_TYPE)
            .in_context(CONTEXT)
            .time(TimeField::Start, Op::Lt, to)
            // zero-length events end at their start, so use >= on the end
            .time(TimeField::End, Op::Ge, from)
            .ordered(Order::StartAsc);
        let mut out = vec![];
        for o in self.queries.query(&q).await? {
            let ev = Event::from_value(o.content.clone())?;
            for occ in expand_with(&*self.engine, &ev, from, to, &self.floating, usize::MAX)? {
                out.push(Instance {
                    object: o.envelope.id,
                    occurrence: occ,
                });
            }
        }
        out.sort_by_key(|i| (i.occurrence.start_utc, i.object));
        Ok(out)
    }

    /// Instances starting from `now` for the next `days` days.
    pub async fn upcoming(&self, now: DateTime<Utc>, days: i64) -> Result<Vec<Instance>> {
        self.occurrences(now, now + Duration::days(days)).await
    }

    /// Tasks whose due date lies in `[from, to)` (range query on the `due_utc` index field).
    pub async fn tasks_due(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<StoredObject>> {
        let q = Query::new()
            .of_type(TASK_TYPE)
            .in_context(CONTEXT)
            .index_between("due_utc", fmt_utc(from), fmt_utc(to))
            .order_by_index("due_utc");
        Ok(self.queries.query(&q).await?)
    }
}
