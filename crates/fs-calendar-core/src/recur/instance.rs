//! One instance of a series: the master with its override applied.
//!
//! Pattern: Template method (one step of `expand`) — build the instance for a key, apply the override
//! patch, read it back as an event and place it in UTC. Any failure belongs to this key alone.

use chrono::{DateTime, NaiveDateTime, Utc};
use serde_json::{Map, Value};

use super::occurrence::{Occurrence, duration_of, zone_of};
use super::patch::apply_patch;
use crate::error::Result;
use crate::model::Event;
use crate::time::{Zone, fmt_local, parse_local};

/// The event as the base of every instance: without the rule and the overrides themselves.
pub(crate) fn master_value(e: &Event) -> Value {
    let mut m = e.to_value().as_object().cloned().unwrap_or_default();
    for k in ["recurrenceRule", "recurrenceOverrides"] {
        m.remove(k);
    }
    Value::Object(m)
}

/// The override patch of a key, if it is an object.
pub(super) fn override_of<'a>(e: &'a Event, key: &str) -> Option<&'a Map<String, Value>> {
    e.recurrence_overrides.get(key).and_then(Value::as_object)
}

/// Whether the override removes the instance.
pub(super) fn is_excluded(ov: Option<&Map<String, Value>>) -> bool {
    ov.is_some_and(|o| o.get("excluded") == Some(&Value::Bool(true)))
}

/// The instance for `local` (the rule's start or an override key), with its UTC span, or `None` if it
/// does not overlap `[from, to)`.
pub(super) fn instance(
    master: &Value,
    ov: Option<&Map<String, Value>>,
    local: NaiveDateTime,
    window: (DateTime<Utc>, DateTime<Utc>),
    floating: &Zone,
) -> Result<Option<Occurrence>> {
    let key = fmt_local(local);
    let mut inst = master.clone();
    let io = inst.as_object_mut().expect("the master is an object");
    io.insert("recurrenceId".into(), Value::String(key.clone()));
    io.insert("start".into(), Value::String(key.clone()));
    if let Some(o) = ov {
        let mut patch = o.clone();
        patch.remove("excluded");
        inst = apply_patch(inst, &patch);
    }
    let ev = Event::from_value(inst)?;
    // a patched instance may move itself
    let start_local = ev
        .start
        .as_deref()
        .map(parse_local)
        .transpose()?
        .unwrap_or(local);
    let dur = duration_of(&ev)?;
    let zone = zone_of(&ev, floating)?;
    let (start_utc, end_utc) = (zone.to_utc(start_local), dur.end_utc(&zone, start_local));
    let (from, to) = window;
    let overlaps = if end_utc > start_utc {
        start_utc < to && end_utc > from
    } else {
        start_utc >= from && start_utc < to
    };
    Ok(overlaps.then_some(Occurrence {
        recurrence_id: key,
        start_utc,
        end_utc,
        event: ev,
        overridden: ov.is_some(),
    }))
}
