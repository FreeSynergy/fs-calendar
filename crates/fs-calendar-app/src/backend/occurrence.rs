//! One occurrence of a series: change and delete as an override, never as a change of the series.
//!
//! Pattern: Command (continued from `commands.rs`) — an id `urn:uuid:<id>#<recurrence id>` names one instance. A change
//! becomes the override `recurrenceOverrides[<recurrence id>]` (a PatchObject of top-level properties, RFC 8984): the
//! properties that differ from the series after the edit; a delete becomes `{"excluded": true}`. The master keeps its
//! start, its rule and its other instances. Members of an old override this code cannot show (paths such as
//! `participants/p1/...`) stay as they are.

use fs_admin_frontend::{BackendError, Outcome, Values};
use fs_calendar_core::{Event, Revision};
use fs_objects::ObjectId;
use serde_json::{Map, Value};

use super::EventBackend;
use super::commands::done;
use super::errors::{refusal, unreadable};
use super::record::apply;

/// Properties that belong to the series or to the store, never to an override.
const NOT_OVERRIDDEN: &[&str] = &[
    "@type",
    "version",
    "uid",
    "prodId",
    "created",
    "updated",
    "sequence",
    "recurrenceRule",
    "recurrenceOverrides",
    "recurrenceId",
    "recurrenceIdTimeZone",
];

/// The recurrence id of the instance an id names, if the id names one.
pub(crate) fn key_of(input: &Values) -> Option<String> {
    let text = input.get("id")?.first()?;
    text.split_once('#').map(|(_, key)| key.to_owned())
}

fn object(v: Value) -> Map<String, Value> {
    match v {
        Value::Object(m) => m,
        _ => Map::new(),
    }
}

/// The master with the top-level members of `patch` applied (nested paths are left for the engine).
fn with_patch(master: &Map<String, Value>, patch: &Map<String, Value>) -> Map<String, Value> {
    let mut out = master.clone();
    for (k, v) in patch.iter().filter(|(k, _)| !k.contains('/')) {
        if v.is_null() {
            out.remove(k);
        } else {
            out.insert(k.clone(), v.clone());
        }
    }
    out
}

impl EventBackend {
    /// Whether the id names one instance of a series stored here.
    pub(crate) fn is_occurrence(
        &self,
        id: ObjectId,
        input: &Values,
    ) -> Result<Option<String>, BackendError> {
        let Some(key) = key_of(input) else {
            return Ok(None);
        };
        let master = self
            .block(self.calendar.get_event(id))
            .map_err(|e| refusal(&self.description, &e))?;
        Ok(master.filter(|e| e.recurrence_rule.is_some()).map(|_| key))
    }

    fn write_override(
        &self,
        id: ObjectId,
        key: &str,
        make: impl FnOnce(&Event) -> Result<Map<String, Value>, BackendError>,
    ) -> Result<Outcome, BackendError> {
        let fail = |e| refusal(&self.description, &e);
        let stored = self
            .block(self.calendar.get(id))
            .map_err(fail)?
            .ok_or(BackendError::NotFound)?;
        let mut master = Event::from_value(stored.content.clone()).map_err(|e| fail(e.into()))?;
        let new = make(&master)?;
        master
            .recurrence_overrides
            .insert(key.to_owned(), Value::Object(new));
        let expected = self.revision(id).unwrap_or_else(|| Revision::of(&stored));
        let written = self
            .block(self.calendar.update_checked(id, &expected, master))
            .map_err(fail)?;
        self.remember(id, Revision::of(&written));
        Ok(done(id))
    }

    /// Change one instance: the edit goes to the override.
    pub(crate) fn change_occurrence(
        &self,
        id: ObjectId,
        key: &str,
        input: &Values,
    ) -> Result<Outcome, BackendError> {
        let values = self.edited(input, true)?;
        self.write_override(id, key, |master| {
            let master_json = object(master.to_value());
            let old = master
                .recurrence_overrides
                .get(key)
                .cloned()
                .map(object)
                .unwrap_or_default();
            let baseline = with_patch(&master_json, &old);
            let mut instance = Event::from_value(Value::Object(baseline.clone()))
                .map_err(|e| refusal(&self.description, &e.into()))?;
            apply(&mut instance, &values).map_err(unreadable)?;
            let edited = object(instance.to_value());
            let mut patch = old;
            for k in baseline
                .keys()
                .chain(edited.keys())
                .filter(|k| !NOT_OVERRIDDEN.contains(&k.as_str()))
            {
                if baseline.get(k) != edited.get(k) {
                    patch.insert(k.clone(), edited.get(k).cloned().unwrap_or(Value::Null));
                }
            }
            Ok(patch)
        })
    }

    /// Delete one instance: it is excluded, the series stays.
    pub(crate) fn delete_occurrence(
        &self,
        id: ObjectId,
        key: &str,
    ) -> Result<Outcome, BackendError> {
        self.write_override(id, key, |_| {
            Ok(Map::from_iter([("excluded".to_owned(), Value::Bool(true))]))
        })
        .map(|_| Outcome::default())
    }
}
