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
            .time(TimeField::End, Op::Ge, from);
        let mut out = Window::default();
        if to <= from {
            return Ok(out);
        }
        // Bytes the instances of this answer may still weigh; each event gets what is left, so what is
        // built is bounded before it is built.
        let mut budget = MAX_ANSWER_BYTES;
        // The candidates are read a page at a time, so the rows of a whole month are never held at
        // once; a page is as many events as fit `PAGE_WEIGHT_BYTES` by what the last page weighed. The
        // next page starts after the `uid_key` of the last row (unique, so no two rows tie; an offset
        // would make the store read every row before it).
        let (mut after, mut page) = (None::<String>, FIRST_PAGE_EVENTS);
        loop {
            let mut next = q.clone().order_by_index("uid_key");
            if let Some(key) = &after {
                next = next.index_op("uid_key", Op::Gt, key.as_str());
            }
            let rows = self.queries.query(&next.limit(page)).await?;
            let mut heaviest = 1usize;
            for o in &rows {
                heaviest = heaviest.max(self.add_event(&mut out, &mut budget, o, (from, to)));
            }
            let last = rows
                .last()
                .and_then(|o| o.index.get("uid_key")?.first().cloned());
            match last {
                Some(key) if rows.len() == page => after = Some(key),
                _ => break,
            }
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
            // A window too long for a rule that could be written today is a cut of the answer; a rule
            // that could not be written today (older data) costs that event only.
            Err(crate::error::Error::Limit(why)) => match o.content.get("recurrenceRule") {
                Some(rule) if is_too_expensive(rule) => {
                    out.damaged
                        .push(damaged(None, format!("the rule is too expensive: {why}")));
                }
                _ => out.damaged.push(cut(format!("cut: {why}"))),
            },
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

/// Whether the rule in this JSON could not be written today (`check_density`).
fn is_too_expensive(rule: &serde_json::Value) -> bool {
    serde_json::from_value::<crate::model::RecurrenceRule>(rule.clone())
        .is_ok_and(|rule| crate::recur::check_density(&rule).is_err())
}
