//! Event -> stored object: envelope, index fields, time span.
//!
//! Pattern: Adapter — maps the JSCalendar event onto the AS2 `Event` envelope and the
//! index fields `fs-objects` can query (ADR 0020, 0032); an unbounded series ends at the `FOREVER` sentinel.

use fs_objects::{Blob, Envelope, ObjectId, StoredObject};

use super::consts::{CONTENT_TYPE, CONTEXT, EVENT_TYPE};
use super::handle::CalendarRepo;
use super::span::index_time;
use crate::error::{Error, Result};
use crate::limits::{MAX_EVENT_BYTES, MAX_EVENT_DEPTH, MAX_EVENT_MEMORY_BYTES, MAX_EVENT_NODES};
use crate::model::Event;
use crate::model::{sensitive_categories_of, shape};
use crate::recur::check_density;
use crate::time::{FOREVER, parse_utc};
use crate::uid::normalise_uid;
use crate::validate::validate;

impl CalendarRepo {
    pub(super) fn build(
        &self,
        obj: &Event,
        existing: Option<&Envelope>,
        original: Option<Blob>,
        rule_is_new: bool,
    ) -> Result<StoredObject> {
        validate(obj)?;
        // Only a rule that is written now is held to today's density; an edit that leaves it alone is not
        // refused for it.
        if let (true, Some(rule)) = (rule_is_new, &obj.recurrence_rule) {
            check_density(rule)?;
        }
        let mut env = existing
            .cloned()
            .unwrap_or_else(|| self.fresh_envelope(obj));
        env.name = obj.display_title(); // ADR 0032: JSCalendar `title`
        // One copy of the event as a value serves the size, the memory, the marking and the store.
        let value = obj.to_value();
        let size = shape(&value);
        if size.memory > MAX_EVENT_MEMORY_BYTES
            || size.nodes > MAX_EVENT_NODES
            || size.depth > MAX_EVENT_DEPTH
        {
            return Err(Error::TooHeavy(format!(
                "{} values, {} levels, about {} bytes in memory; at most {MAX_EVENT_NODES}, {MAX_EVENT_DEPTH}, {MAX_EVENT_MEMORY_BYTES}",
                size.nodes, size.depth, size.memory
            )));
        }
        let bytes = json_len(&value) + env.name.len();
        if bytes > MAX_EVENT_BYTES {
            return Err(Error::TooLarge(bytes));
        }
        // ADR 0029: the marking in the content reaches the envelope; an update never takes one away.
        env.sensitive.extend(sensitive_categories_of(&value));
        env.sensitive.sort();
        env.sensitive.dedup();
        let span = match self.time_index(obj) {
            Ok(span) => span,
            // A series older than the limits whose end cannot be computed: an edit that leaves its rule
            // alone is not refused for that. It stays a candidate for every window from its start on.
            Err(Error::Limit(_) | Error::Recurrence(_)) if !rule_is_new && existing.is_some() => {
                self.open_ended(obj)?
            }
            Err(e) => return Err(e),
        };
        // AS2 startTime / endTime; an unbounded series ends at the FOREVER sentinel.
        env.start_time = Some(span.start);
        env.end_time = match span.end {
            Some(e) => Some(e),
            None if span.unbounded => Some(parse_utc(FOREVER)?),
            None => None,
        };
        let mut o = StoredObject::new(env, CONTENT_TYPE, value);
        o.original = original;
        let id = o.envelope.id;
        let mut put = |k: &str, v: String| o.index.entry(k.into()).or_default().push(v);
        put("uid", obj.uid.clone());
        put("uid_key", normalise_uid(&obj.uid));
        // The key the candidates are paged by: lower case and unique, so the store sorts and compares it alike.
        put(super::consts::PAGE_KEY, id.to_string());
        put("calendar_id", "default".into());
        put("type", "event".into());
        put("start_utc", index_time(span.start));
        match span.end {
            Some(e) => put("end_utc", index_time(e)),
            None if span.unbounded => put("end_utc", FOREVER.into()),
            None => {}
        }
        if obj.recurrence_rule.is_some() || !obj.recurrence_overrides.is_empty() {
            put("recurring", "true".into());
        }
        if let Some(s) = &obj.status {
            put("status", s.clone());
        }
        put("sequence", obj.sequence.unwrap_or(0).to_string());
        for p in obj.participants.values() {
            if let Some(m) = p.email.as_deref().or(p.calendar_address.as_deref()) {
                put(
                    "participant_email",
                    m.trim().trim_start_matches("mailto:").to_lowercase(),
                );
            }
        }
        Ok(o)
    }

    fn fresh_envelope(&self, obj: &Event) -> Envelope {
        let mut e = Envelope::new(EVENT_TYPE, obj.display_title());
        if let Ok(id) = normalise_uid(&obj.uid).parse::<ObjectId>() {
            e.id = id;
        }
        e.iri = Some(format!("https://{}/events/{}", self.instance, e.id));
        e.context = Some(CONTEXT.into());
        e.attributed_to = Some(self.actor.clone());
        e.to = vec![self.actor.clone()];
        e
    }
}

/// Counts the bytes written to it and keeps none.
struct Counting(usize);

impl std::io::Write for Counting {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0 += buf.len();
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// The length of the JSON text of a value, without making the text.
fn json_len(value: &serde_json::Value) -> usize {
    let mut count = Counting(0);
    // A value always serialises; counting cannot fail.
    let _ = serde_json::to_writer(&mut count, value);
    count.0
}
