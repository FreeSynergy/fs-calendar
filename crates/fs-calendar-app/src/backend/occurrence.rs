//! One occurrence of a series: change and delete as an override, never as a change of the series.
//!
//! Pattern: Command (continued from `commands.rs`) — an id `urn:uuid:<id>#<recurrence id>` names one instance. A change
//! becomes the override `recurrenceOverrides[<recurrence id>]` (a PatchObject of top-level properties, RFC 8984): the
//! properties that differ from the series after the edit; a delete becomes `{"excluded": true}`. The master keeps its
//! start, its rule and its other instances. Members of an old override this code cannot show (paths such as
//! `participants/p1/...`) stay as they are.

use chrono::Duration;
use fs_admin_frontend::{BackendError, Outcome, Values};
use fs_calendar_core::time::{Zone, parse_local};
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
    /// The recurrence id if the id names one instance of a series stored here.
    ///
    /// # Errors
    /// `NotFound` when there is such a key but no such instance (a day the series does not have, an excluded
    /// instance, a key that is no date, an event that is no series): nothing is written for it.
    pub(crate) fn is_occurrence(
        &self,
        id: ObjectId,
        input: &Values,
    ) -> Result<Option<String>, BackendError> {
        let Some(key) = key_of(input) else {
            return Ok(None);
        };
        let fail = |e| refusal(&self.description, &e);
        let master = self
            .block(self.calendar.get_event(id))
            .map_err(fail)?
            .ok_or(BackendError::NotFound)?;
        if master.recurrence_rule.is_none() && !master.recurrence_overrides.contains_key(&key) {
            // The core gives every instance a recurrence id, a single event's too (its own start): that id is the
            // event itself. Any other key names nothing.
            return if master.start.as_deref() == Some(key.as_str()) {
                Ok(None)
            } else {
                Err(BackendError::NotFound)
            };
        }
        let added = master
            .recurrence_overrides
            .get(&key)
            .is_some_and(|o| o.get("excluded") != Some(&Value::Bool(true)));
        if added || self.is_instance(id, &master, &key)? {
            Ok(Some(key))
        } else {
            Err(BackendError::NotFound)
        }
    }

    /// Whether the series has an instance with this recurrence id (asked of the core, which knows the rule).
    fn is_instance(&self, id: ObjectId, master: &Event, key: &str) -> Result<bool, BackendError> {
        let Ok(local) = parse_local(key) else {
            return Ok(false);
        };
        let zone = master
            .time_zone
            .as_deref()
            .and_then(|z| Zone::parse(z).ok())
            .unwrap_or(Zone::Utc);
        let at = zone.to_utc(local);
        // a day of margin each way: a floating series is read in the zone of the program, not known here
        let found = self
            .block(
                self.calendar
                    .window(at - Duration::days(1), at + Duration::days(2)),
            )
            .map_err(|e| refusal(&self.description, &e))?;
        Ok(found
            .instances
            .iter()
            .any(|i| i.object == id && i.occurrence.recurrence_id == key))
    }

    fn write_override(
        &self,
        id: ObjectId,
        key: &str,
        make: impl FnOnce(&Event) -> Result<Option<Map<String, Value>>, BackendError>,
    ) -> Result<Outcome, BackendError> {
        let fail = |e| refusal(&self.description, &e);
        let stored = self
            .block(self.calendar.get(id))
            .map_err(fail)?
            .ok_or(BackendError::NotFound)?;
        let mut master = Event::from_value(stored.content.clone()).map_err(|e| fail(e.into()))?;
        // `None`: nothing differs, nothing is written and the revision stays. An empty override is not kept: the
        // instance is the series' own again.
        let Some(new) = make(&master)? else {
            return Ok(done(id));
        };
        if new.is_empty() {
            master.recurrence_overrides.remove(key);
        } else {
            master
                .recurrence_overrides
                .insert(key.to_owned(), Value::Object(new));
        }
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
            // The instance starts at its key (the series' start only for the first one) unless an override moved it:
            // the edit is compared with that, so a move back to the start of the series is a difference.
            let mut baseline = with_patch(&master_json, &old);
            if !old.contains_key("start") {
                baseline.insert("start".to_owned(), Value::String(key.to_owned()));
            }
            let mut instance = Event::from_value(Value::Object(baseline.clone()))
                .map_err(|e| refusal(&self.description, &e.into()))?;
            apply(&mut instance, &values).map_err(unreadable)?;
            let edited = object(instance.to_value());
            let mut patch = old.clone();
            for k in baseline
                .keys()
                .chain(edited.keys())
                .filter(|k| !NOT_OVERRIDDEN.contains(&k.as_str()))
            {
                if baseline.get(k) != edited.get(k) {
                    patch.insert(k.clone(), edited.get(k).cloned().unwrap_or(Value::Null));
                }
            }
            // a start equal to the key is the default, not an override
            if patch.get("start").and_then(Value::as_str) == Some(key) {
                patch.remove("start");
            }
            Ok((patch != old).then_some(patch))
        })
    }

    /// Delete one instance: it is excluded, the series stays.
    pub(crate) fn delete_occurrence(
        &self,
        id: ObjectId,
        key: &str,
    ) -> Result<Outcome, BackendError> {
        self.write_override(id, key, |_| {
            Ok(Some(Map::from_iter([(
                "excluded".to_owned(),
                Value::Bool(true),
            )])))
        })
        .map(|_| Outcome::default())
    }
}
