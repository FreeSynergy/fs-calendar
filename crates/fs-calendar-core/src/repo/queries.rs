//! Listing and time-range queries.
//!
//! Pattern: Repository (read side) + Iterator-style expansion — the store selects candidates by
//! the envelope times and index fields; only those are expanded into instances by the
//! recurrence engine.

use chrono::{DateTime, Utc};
use fs_store::{Op, Order, Query, StoredObject, TimeField};

use super::consts::{CONTEXT, EVENT_TYPE};
use super::handle::CalendarRepo;
use super::types::{Damaged, Instance, Window};
use crate::error::Result;
use crate::limits::{
    FIRST_PAGE_EVENTS, INSTANCE_OVERHEAD_BYTES, MAX_ANSWER_BYTES, MAX_INSTANCES_PER_EVENT,
    MAX_PAGE_EVENTS, PAGE_WEIGHT_BYTES,
};
use crate::model::{Event, memory_weight};
use crate::recur::expand_lenient;

impl CalendarRepo {
    /// All events, ordered by first start (by the store).
    pub async fn list_events(&self) -> Result<Vec<StoredObject>> {
        let q = Query::new()
            .of_type(EVENT_TYPE)
            .in_context(CONTEXT)
            .ordered(Order::StartAsc);
        Ok(self.queries.query(&q).await?)
    }

    /// Event instances overlapping `[from, to)`, recurring series expanded at query time,
    /// ordered by start. What could not be answered is left out here; [`CalendarRepo::window`] names it.
    ///
    /// The store selects the candidates with a range query on the envelope times
    /// (`start_time < to AND end_time >= from`; a series carries its last end, an unbounded one
    /// the `FOREVER` sentinel, and starts at its earliest instance), and only those are expanded.
    pub async fn occurrences(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<Instance>> {
        Ok(self.window(from, to).await?.instances)
    }

    /// Like [`CalendarRepo::occurrences`], and names what it could not answer: an override that does
    /// not read costs its own instance, an event whose rule or zone does not read costs itself, a
    /// window too far from a series start costs that series, and more than `MAX_INSTANCES_PER_EVENT`
    /// instances of one event are cut. Everything else is answered.
    pub async fn window(&self, from: DateTime<Utc>, to: DateTime<Utc>) -> Result<Window> {
        let q = Query::new()
            .of_type(EVENT_TYPE)
            .in_context(CONTEXT)
            .time(TimeField::Start, Op::Lt, to)
            // zero-length events end at their start, so use >= on the end
            .time(TimeField::End, Op::Ge, from)
            .ordered(Order::StartAsc);
        let mut out = Window::default();
        if to <= from {
            return Ok(out);
        }
        // Bytes the instances of this answer may still weigh; each event gets what is left, so what is
        // built is bounded before it is built.
        let mut budget = MAX_ANSWER_BYTES;
        // The candidates are read a page at a time, so the rows of a whole month are never held at
        // once; a page is as many events as fit `PAGE_WEIGHT_BYTES` by what the last page weighed.
        let (mut offset, mut page) = (0usize, FIRST_PAGE_EVENTS);
        loop {
            let rows = self
                .queries
                .query(&q.clone().limit(page).offset(offset))
                .await?;
            if rows.is_empty() {
                break;
            }
            let count = rows.len();
            let mut heaviest = 1usize;
            for o in rows {
                heaviest = heaviest.max(self.add_event(&mut out, &mut budget, &o, (from, to)));
            }
            if count < page {
                break;
            }
            offset += count;
            page = (PAGE_WEIGHT_BYTES / heaviest).clamp(1, MAX_PAGE_EVENTS);
        }
        out.instances
            .sort_by_key(|i| (i.occurrence.start_utc, i.object));
        Ok(out)
    }

    /// Expand one event into `out`, within what is left of `budget`; answers what its instance weighs.
    fn add_event(
        &self,
        out: &mut Window,
        budget: &mut usize,
        o: &StoredObject,
        window: (DateTime<Utc>, DateTime<Utc>),
    ) -> usize {
        let (from, to) = window;
        let object = o.envelope.id;
        let damaged = |key: Option<String>, reason: String| Damaged {
            object,
            key,
            reason,
            cut: false,
        };
        let cut = |reason: String| Damaged {
            cut: true,
            ..damaged(None, reason)
        };
        let weight = memory_weight(&o.content) + INSTANCE_OVERHEAD_BYTES;
        let limit = MAX_INSTANCES_PER_EVENT.min(*budget / weight);
        if limit == 0 {
            out.damaged.push(cut(format!(
                "cut: the answer holds at most {MAX_ANSWER_BYTES} bytes of instances"
            )));
            return weight;
        }
        let expanded = Event::from_value(o.content.clone())
            .map_err(crate::error::Error::from)
            .and_then(|ev| expand_lenient(&*self.engine, &ev, from, to, &self.floating, limit));
        match expanded {
            Err(crate::error::Error::Limit(why)) => {
                out.damaged.push(cut(format!("cut: {why}")));
            }
            Err(e) => out.damaged.push(damaged(None, e.to_string())),
            Ok(found) => {
                out.damaged.extend(
                    found
                        .damaged
                        .into_iter()
                        .map(|d| damaged(Some(d.key), d.reason)),
                );
                if found.cut {
                    out.damaged.push(cut(format!(
                        "cut: more than {limit} instances of {weight} bytes fit the window"
                    )));
                }
                *budget = budget.saturating_sub(found.items.len() * weight);
                out.instances.extend(
                    found
                        .items
                        .into_iter()
                        .map(|occurrence| Instance { object, occurrence }),
                );
            }
        }
        weight
    }

    /// Instances starting from `now` for the next `days` days.
    pub async fn upcoming(&self, now: DateTime<Utc>, days: i64) -> Result<Vec<Instance>> {
        self.occurrences(now, crate::limits::after_days(now, days))
            .await
    }
}
