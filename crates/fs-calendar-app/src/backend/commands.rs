//! The commands that write one event: create, change, delete.
//!
//! Pattern: Command dispatch with Template method — `command` finds the handler by name; every handler reads the id
//! the same way (`event_id`, a fragment of an occurrence is dropped: the series is the record) and answers with
//! `Outcome`. `change` is partial and goes through `update_checked` with the revision the list showed.

use std::str::FromStr;

use fs_admin_frontend::{BackendError, Outcome, Values};
use fs_calendar_core::{Event, Revision};
use fs_objects::ObjectId;

use super::errors::{bad_id, missing, refusal, unreadable};
use super::record::apply;
use super::{EventBackend, iri};

/// The event an id names: `urn:uuid:<id>` with or without `#<occurrence>`.
pub(crate) fn event_id(input: &Values) -> Result<ObjectId, BackendError> {
    let text = input
        .get("id")
        .and_then(|v| v.first())
        .ok_or_else(|| missing("id"))?;
    let base = text.split('#').next().unwrap_or("");
    ObjectId::from_str(base).map_err(|_| bad_id())
}

pub(crate) fn done(id: ObjectId) -> Outcome {
    Outcome {
        id: Some(iri(id)),
        values: Values::new(),
    }
}

impl EventBackend {
    pub(crate) fn command(&self, name: &str, input: &Values) -> Result<Outcome, BackendError> {
        let fail = |e| refusal(&self.description, &e);
        match name {
            "create" => {
                let mut event = Event::blank();
                apply(&mut event, input).map_err(unreadable)?;
                let stored = self.block(self.calendar.create(event)).map_err(fail)?;
                self.remember(stored.envelope.id, Revision::of(&stored));
                Ok(done(stored.envelope.id))
            }
            "change" => {
                let id = event_id(input)?;
                let stored = self
                    .block(self.calendar.get(id))
                    .map_err(fail)?
                    .ok_or(BackendError::NotFound)?;
                let mut event =
                    Event::from_value(stored.content.clone()).map_err(|e| fail(e.into()))?;
                apply(&mut event, input).map_err(unreadable)?;
                let expected = self.revision(id).unwrap_or_else(|| Revision::of(&stored));
                let written = self
                    .block(self.calendar.update_checked(id, &expected, event))
                    .map_err(fail)?;
                self.remember(id, Revision::of(&written));
                Ok(done(id))
            }
            "delete" => {
                let id = event_id(input)?;
                self.block(self.calendar.delete(id)).map_err(fail)?;
                Ok(Outcome::default())
            }
            "find" => {
                let found = self.find(input)?;
                let ids = found.into_iter().map(|r| r.id).collect();
                Ok(Outcome {
                    id: None,
                    values: Values::from([("events".to_owned(), ids)]),
                })
            }
            other => self.transfer_command(other, input),
        }
    }
}
