//! Calendar object -> stored object: envelope, index fields, time span.
//!
//! Pattern: Adapter — maps the JSCalendar object onto the AS2 `Event` / `Task` envelope and the
//! index fields `fs-store` can query (ADR 0020, 0032); an unbounded series ends at the `FOREVER` sentinel.

use fs_store::{Blob, Envelope, ObjectId, StoredObject};

use super::consts::{CONTENT_TYPE, CONTEXT};
use super::handle::CalendarRepo;
use crate::error::Result;
use crate::model::CalendarObject;
use crate::time::{FOREVER, fmt_utc, parse_utc};
use crate::validate::validate;

impl CalendarRepo {
    pub(super) fn build(
        &self,
        obj: &CalendarObject,
        existing: Option<&Envelope>,
        original: Option<Blob>,
    ) -> Result<StoredObject> {
        validate(obj)?;
        let mut env = existing
            .cloned()
            .unwrap_or_else(|| self.fresh_envelope(obj));
        env.name = obj.title(); // ADR 0032: JSCalendar `title`
        let span = self.time_index(obj)?;
        // AS2 startTime / endTime; an unbounded series ends at the FOREVER sentinel.
        env.start_time = span.start;
        env.end_time = match span.end {
            Some(e) => Some(e),
            None if span.unbounded => Some(parse_utc(FOREVER)?),
            None => None,
        };
        let mut o = StoredObject::new(env, CONTENT_TYPE, obj.to_value());
        o.original = original;
        let mut put = |k: &str, v: String| o.index.entry(k.into()).or_default().push(v);
        put("uid", obj.uid().to_string());
        put("calendar_id", "default".into());
        put("type", obj.type_name().to_lowercase());
        if let Some(s) = span.start {
            put("start_utc", fmt_utc(s));
        }
        match span.end {
            Some(e) => put("end_utc", fmt_utc(e)),
            None if span.unbounded => put("end_utc", FOREVER.into()),
            None => {}
        }
        if let Some(d) = span.due {
            put("due_utc", fmt_utc(d));
        }
        if let CalendarObject::Event(e) = obj {
            if e.recurrence_rule.is_some() || !e.recurrence_overrides.is_empty() {
                put("recurring", "true".into());
            }
            if let Some(s) = &e.status {
                put("status", s.clone());
            }
            put("sequence", e.sequence.unwrap_or(0).to_string());
        }
        if let CalendarObject::Task(t) = obj {
            if let Some(p) = &t.progress {
                put("progress", p.clone());
            }
        }
        for p in obj.participants().values() {
            if let Some(m) = p.email.as_deref().or(p.calendar_address.as_deref()) {
                put(
                    "participant_email",
                    m.trim().trim_start_matches("mailto:").to_lowercase(),
                );
            }
        }
        Ok(o)
    }

    fn fresh_envelope(&self, obj: &CalendarObject) -> Envelope {
        let mut e = Envelope::new(obj.type_name(), obj.title());
        if let Ok(id) = obj.uid().parse::<ObjectId>() {
            e.id = id;
        }
        e.iri = Some(format!("https://{}/events/{}", self.instance, e.id));
        e.context = Some(CONTEXT.into());
        e.attributed_to = Some(self.actor.clone());
        e.to = vec![self.actor.clone()];
        e
    }
}
