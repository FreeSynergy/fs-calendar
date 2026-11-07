//! Listing and time-range queries.
//!
//! Pattern: Repository (read side) + Iterator-style expansion — the store selects candidates by
//! the envelope times and index fields; only those are expanded into instances by the
//! recurrence engine.

use chrono::{DateTime, Duration, Utc};
use fs_store::{Op, Order, Query, StoredObject, TimeField};

use super::consts::{CONTEXT, EVENT_TYPE};
use super::handle::CalendarRepo;
use super::types::{Damaged, Instance, Window};
use crate::error::Result;
use crate::limits::MAX_INSTANCES_PER_EVENT;
use crate::model::Event;
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
        for o in self.queries.query(&q).await? {
            let object = o.envelope.id;
            let damaged = |key: Option<String>, reason: String| Damaged {
                object,
                key,
                reason,
            };
            let expanded = Event::from_value(o.content.clone())
                .map_err(crate::error::Error::from)
                .and_then(|ev| {
                    expand_lenient(
                        &*self.engine,
                        &ev,
                        from,
                        to,
                        &self.floating,
                        MAX_INSTANCES_PER_EVENT,
                    )
                });
            match expanded {
                Err(e) => out.damaged.push(damaged(None, e.to_string())),
                Ok(found) => {
                    out.damaged.extend(
                        found
                            .damaged
                            .into_iter()
                            .map(|d| damaged(Some(d.key), d.reason)),
                    );
                    if found.cut {
                        out.damaged.push(damaged(
                            None,
                            format!(
                                "cut: more than {MAX_INSTANCES_PER_EVENT} instances in the window"
                            ),
                        ));
                    }
                    out.instances.extend(
                        found
                            .items
                            .into_iter()
                            .map(|occurrence| Instance { object, occurrence }),
                    );
                }
            }
        }
        out.instances
            .sort_by_key(|i| (i.occurrence.start_utc, i.object));
        Ok(out)
    }

    /// Instances starting from `now` for the next `days` days.
    pub async fn upcoming(&self, now: DateTime<Utc>, days: i64) -> Result<Vec<Instance>> {
        self.occurrences(now, now + Duration::days(days)).await
    }
}
