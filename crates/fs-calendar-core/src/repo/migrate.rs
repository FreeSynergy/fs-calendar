//! Migration of objects written before fs-store 0.2.
//!
//! Pattern: Command (idempotent batch) — rewrites each old object once so its envelope times
//! match the index fields; running it again changes nothing.

use fs_store::{Cond, Op, Query};

use super::consts::{CONTEXT, EVENT_TYPE};
use super::handle::CalendarRepo;
use crate::error::Result;
use crate::model::Event;

impl CalendarRepo {
    /// Fill `start_time` / `end_time` of objects written before fs-store 0.2 (they only have the
    /// `start_utc` / `end_utc` index fields). Returns the number of objects rewritten. Idempotent.
    pub async fn migrate_time_fields(&self) -> Result<usize> {
        let q = Query::new().in_context(CONTEXT).and(Cond::Index {
            key: "start_utc".into(),
            op: Op::Ge,
            value: String::new(),
        });
        let _one_at_a_time = self.writes.lock().await;
        let mut n = 0;
        for o in self.queries.query(&q).await? {
            let known = o.envelope.object_type == EVENT_TYPE;
            if o.envelope.start_time.is_some() || !known {
                continue;
            }
            let obj = Event::from_value(o.content.clone())?;
            let mut fresh = self.build(&obj, Some(&o.envelope), None)?;
            // keep non-derived index fields of the original import
            if let Some(v) = o.index.get("tzdb_version") {
                fresh.index.insert("tzdb_version".into(), v.clone());
            }
            self.objects.put(fresh, &self.actor).await?;
            n += 1;
        }
        Ok(n)
    }
}
