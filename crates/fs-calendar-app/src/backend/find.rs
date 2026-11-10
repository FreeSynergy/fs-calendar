//! `find`: the list, a search, or the occurrences of a range.
//!
//! Pattern: Adapter — a list is the command kind `find` (decision of the strategy). Without `from` and `to` the
//! events come in the order of the core (first start); with both, the instances of that range, a series expanded,
//! each with an id `urn:uuid:<id>#<recurrence id>`. A search `term` looks in title and description. Damaged events
//! the core names (`Window.damaged`) are left out; a gap told in `DESIGN.md`.

use chrono::{DateTime, Utc};
use fs_admin_frontend::{BackendError, Record, Values};
use fs_calendar_core::{Event, Revision};

use super::errors::{missing, refusal, unreadable};
use super::record::{Unreadable, to_values};
use super::{EventBackend, instance_id, iri};

fn when(query: &Values, name: &str) -> Result<Option<DateTime<Utc>>, BackendError> {
    match query.get(name).and_then(|v| v.first()) {
        None => Ok(None),
        Some(t) => DateTime::parse_from_rfc3339(t)
            .map(|d| Some(d.with_timezone(&Utc)))
            .map_err(|_| {
                unreadable(Unreadable {
                    field: name.to_owned(),
                    index: 0,
                })
            }),
    }
}

/// The range asked for: both ends or none.
/// A range of time, both ends given.
pub(crate) type Range = (DateTime<Utc>, DateTime<Utc>);

pub(crate) fn range(q: &Values) -> Result<Option<Range>, BackendError> {
    match (when(q, "from")?, when(q, "to")?) {
        (Some(a), Some(b)) => Ok(Some((a, b))),
        (None, None) => Ok(None),
        (Some(_), None) => Err(missing("to")),
        (None, Some(_)) => Err(missing("from")),
    }
}

fn matches(term: Option<&str>, e: &Event) -> bool {
    let Some(t) = term.map(str::to_lowercase).filter(|t| !t.is_empty()) else {
        return true;
    };
    [&e.title, &e.description]
        .iter()
        .any(|f| f.as_deref().is_some_and(|s| s.to_lowercase().contains(&t)))
}

impl EventBackend {
    pub(crate) fn find(&self, query: &Values) -> Result<Vec<Record>, BackendError> {
        let term = query
            .get("term")
            .and_then(|v| v.first())
            .map(String::as_str);
        let fail = |e| refusal(&self.description, &e);
        if let Some((from, to)) = range(query)? {
            // The revisions are read BEFORE the instances. A write between the two reads then makes a later change a
            // conflict (the person may be shown values newer than the revision, never older), so no update is lost.
            // An event created between the two reads has no remembered revision and is changed at the revision then
            // read: a gap of that one moment, named in DESIGN.md.
            for stored in self.block(self.calendar.list_events()).map_err(fail)? {
                self.remember(stored.envelope.id, Revision::of(&stored));
            }
            let window = self.block(self.calendar.window(from, to)).map_err(fail)?;
            return Ok(window
                .instances
                .into_iter()
                .filter(|i| matches(term, &i.occurrence.event))
                .map(|i| Record {
                    id: instance_id(&i),
                    values: to_values(&i.occurrence.event),
                })
                .collect());
        }
        let stored = self.block(self.calendar.list_events()).map_err(fail)?;
        let mut out = Vec::new();
        for s in stored {
            let Ok(e) = Event::from_value(s.content.clone()) else {
                continue;
            };
            if matches(term, &e) {
                self.remember(s.envelope.id, Revision::of(&s));
                out.push(Record {
                    id: iri(s.envelope.id),
                    values: to_values(&e),
                });
            }
        }
        Ok(out)
    }
}
