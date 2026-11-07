//! Event -> stored object: envelope, index fields, time span.
//!
//! Pattern: Adapter — maps the JSCalendar event onto the AS2 `Event` envelope and the
//! index fields `fs-store` can query (ADR 0020, 0032); an unbounded series ends at the `FOREVER` sentinel.

use fs_store::{Blob, Envelope, ObjectId, StoredObject};

use super::consts::{CONTENT_TYPE, CONTEXT, EVENT_TYPE};
use super::handle::CalendarRepo;
use crate::error::{Error, Result};
use crate::limits::MAX_EVENT_BYTES;
use crate::model::Event;
use crate::time::{FOREVER, fmt_utc, parse_utc};
use crate::uid::normalise_uid;
use crate::validate::validate;

impl CalendarRepo {
    pub(super) fn build(
        &self,
        obj: &Event,
        existing: Option<&Envelope>,
        original: Option<Blob>,
    ) -> Result<StoredObject> {
        validate(obj)?;
        let mut env = existing
            .cloned()
            .unwrap_or_else(|| self.fresh_envelope(obj));
        env.name = obj.display_title(); // ADR 0032: JSCalendar `title`
        let bytes = obj.to_value().to_string().len() + env.name.len();
        if bytes > MAX_EVENT_BYTES {
            return Err(Error::TooLarge(bytes));
        }
        // ADR 0029: the marking in the content reaches the envelope; an update never takes one away.
        env.sensitive.extend(obj.sensitive_categories());
        env.sensitive.sort();
        env.sensitive.dedup();
        let span = self.time_index(obj)?;
        // AS2 startTime / endTime; an unbounded series ends at the FOREVER sentinel.
        env.start_time = Some(span.start);
        env.end_time = match span.end {
            Some(e) => Some(e),
            None if span.unbounded => Some(parse_utc(FOREVER)?),
            None => None,
        };
        let mut o = StoredObject::new(env, CONTENT_TYPE, obj.to_value());
        o.original = original;
        let mut put = |k: &str, v: String| o.index.entry(k.into()).or_default().push(v);
        put("uid", obj.uid.clone());
        put("uid_key", normalise_uid(&obj.uid));
        put("calendar_id", "default".into());
        put("type", "event".into());
        put("start_utc", fmt_utc(span.start));
        match span.end {
            Some(e) => put("end_utc", fmt_utc(e)),
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
