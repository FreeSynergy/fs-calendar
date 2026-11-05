//! Calendar repository over [`fs_store::Store`].

use std::sync::Arc;

use chrono::{DateTime, Duration, Utc};
use fs_store::{ActivityFilter, ActivityKind, Blob, Envelope, LogEntry, ObjectId, Op, Order, Query, Store, StoredObject, TimeField};
use serde_json::{Value, json};

use crate::error::{Error, Result};
use crate::ical::{self, ICS_MEDIA_TYPE};
use crate::model::{CalendarObject, Event, Task};
use crate::recur::{self, Occurrence, first_span};
use crate::time::{Dur, FOREVER, Zone, tzdb_version, fmt_utc, parse_local, parse_utc};
use crate::upgrade::upgrade;
use crate::validate::validate;

pub const CONTEXT: &str = "calendar";
pub const CONTENT_TYPE: &str = "application/jscalendar+json";
pub const EVENT_TYPE: &str = "Event";
pub const TASK_TYPE: &str = "Task";

/// An answer to an invitation (ADR 0020 log mapping).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Response {
    Accept,
    TentativeAccept,
    Reject,
}

impl Response {
    /// AS2 activity type name.
    pub fn as_str(&self) -> &'static str {
        match self {
            Response::Accept => "Accept",
            Response::TentativeAccept => "TentativeAccept",
            Response::Reject => "Reject",
        }
    }
    /// JSCalendar `participationStatus`.
    pub fn status(&self) -> &'static str {
        match self {
            Response::Accept => "accepted",
            Response::TentativeAccept => "tentative",
            Response::Reject => "declined",
        }
    }
    fn kind(&self) -> ActivityKind {
        match self {
            Response::Accept => ActivityKind::Accept,
            Response::TentativeAccept => ActivityKind::TentativeAccept,
            Response::Reject => ActivityKind::Reject,
        }
    }
}

/// One entry of the invitation history of an object.
#[derive(Clone, Debug, PartialEq)]
pub struct InvitationEntry {
    /// `Invite`, `Accept`, `TentativeAccept` or `Reject`.
    pub activity: String,
    pub participant: String,
    pub email: Option<String>,
    pub comment: Option<String>,
    pub at: DateTime<Utc>,
}

/// An event instance with the object it belongs to.
#[derive(Clone, Debug, PartialEq)]
pub struct Instance {
    pub object: ObjectId,
    pub occurrence: Occurrence,
}

#[derive(Clone, Debug, Default)]
pub struct ImportReport {
    pub created: Vec<ObjectId>,
    pub skipped: Vec<String>,
    pub failed: Vec<String>,
    /// What the 1.0 -> 2.0 upgrade changed.
    pub notes: Vec<String>,
}

struct Span {
    start: Option<DateTime<Utc>>,
    end: Option<DateTime<Utc>>,
    unbounded: bool,
    due: Option<DateTime<Utc>>,
}

/// Calendar over an object store.
#[derive(Clone)]
pub struct CalendarRepo {
    store: Arc<dyn Store>,
    actor: String,
    instance: String,
    floating: Zone,
}

impl CalendarRepo {
    pub fn new(store: Arc<dyn Store>, actor: impl Into<String>) -> Self {
        Self { store, actor: actor.into(), instance: "local.invalid".into(), floating: Zone::Utc }
    }

    /// Instance domain for IRIs `https://<instance>/events/<id>` (ADR 0027).
    pub fn with_instance(mut self, instance: impl Into<String>) -> Self {
        self.instance = instance.into();
        self
    }

    /// Zone in which floating events (no `timeZone`) are interpreted for indexing and queries.
    pub fn with_floating_zone(mut self, zone: Zone) -> Self {
        self.floating = zone;
        self
    }

    pub fn store(&self) -> &Arc<dyn Store> {
        &self.store
    }

    // ---- build ------------------------------------------------------------------------------

    /// Time span for the index: `(start, end, unbounded, due)`, all UTC. A series ends at its
    /// last instance, or is unbounded (no `count` / `until`).
    fn time_index(&self, obj: &CalendarObject) -> Result<Span> {
        match obj {
            CalendarObject::Event(e) => {
                let (start, _) = first_span(e, &self.floating)?;
                let end = recur::series_end(e, &self.floating)?;
                Ok(Span { start: Some(start), end, unbounded: end.is_none(), due: None })
            }
            CalendarObject::Task(t) => {
                let zone = match t.time_zone.as_deref() {
                    Some(z) => Zone::parse(z)?,
                    None => self.floating.clone(),
                };
                let local = |s: &Option<String>| -> Result<Option<DateTime<Utc>>> {
                    s.as_deref().map(|x| parse_local(x).map(|l| zone.to_utc(l))).transpose()
                };
                let (start, due) = (local(&t.start)?, local(&t.due)?);
                let est = t.estimated_duration.as_deref().map(Dur::parse).transpose()?;
                let end = due.or_else(|| start.zip(est).map(|(s, d)| d.end_utc(&Zone::Utc, s.naive_utc())));
                Ok(Span { start: start.or(due), end: end.or(start), unbounded: false, due })
            }
        }
    }

    fn build(&self, obj: &CalendarObject, existing: Option<&Envelope>, original: Option<Blob>) -> Result<StoredObject> {
        validate(obj)?;
        let mut env = match existing {
            Some(e) => e.clone(),
            None => {
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
        };
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
                put("participant_email", m.trim().trim_start_matches("mailto:").to_lowercase());
            }
        }
        Ok(o)
    }

    // ---- CRUD -------------------------------------------------------------------------------

    pub async fn create(&self, obj: impl Into<CalendarObject>) -> Result<StoredObject> {
        self.create_inner(obj.into(), None).await
    }

    async fn create_inner(&self, mut obj: CalendarObject, original: Option<Blob>) -> Result<StoredObject> {
        let now = crate::now_utc();
        match &mut obj {
            CalendarObject::Event(e) => {
                e.normalise();
                e.created.get_or_insert_with(|| now.clone());
                e.updated.get_or_insert(now);
            }
            CalendarObject::Task(t) => {
                t.normalise();
                t.created.get_or_insert_with(|| now.clone());
                t.updated.get_or_insert(now);
            }
        }
        if self.get_by_uid(obj.uid()).await?.is_some() {
            return Err(Error::Conflict(format!("an object with uid {} exists", obj.uid())));
        }
        let mut o = self.build(&obj, None, original.clone())?;
        if original.is_some() {
            o.index.entry("tzdb_version".into()).or_default().push(tzdb_version().into());
        }
        let id = o.envelope.id;
        self.store.put(o, &self.actor).await?;
        self.get(id).await?.ok_or(Error::NotFound(id))
    }

    pub async fn get(&self, id: ObjectId) -> Result<Option<StoredObject>> {
        Ok(self.store.get(id).await?.filter(|o| matches!(o.envelope.object_type.as_str(), EVENT_TYPE | TASK_TYPE)))
    }

    pub async fn get_object(&self, id: ObjectId) -> Result<Option<CalendarObject>> {
        self.get(id).await?.map(|o| CalendarObject::from_value(o.content)).transpose()
    }

    pub async fn get_event(&self, id: ObjectId) -> Result<Option<Event>> {
        Ok(match self.get_object(id).await? {
            Some(CalendarObject::Event(e)) => Some(e),
            _ => None,
        })
    }

    pub async fn get_task(&self, id: ObjectId) -> Result<Option<Task>> {
        Ok(match self.get_object(id).await? {
            Some(CalendarObject::Task(t)) => Some(t),
            _ => None,
        })
    }

    pub async fn get_by_uid(&self, uid: &str) -> Result<Option<StoredObject>> {
        let q = Query::new().in_context(CONTEXT).with_index("uid", uid).limit(1);
        Ok(self.store.query(&q).await?.into_iter().next())
    }

    /// Replace the object. `uid` and type cannot change; `updated` is set to now; `sequence` is
    /// incremented when a scheduling-relevant field changed (start, zone, duration, rule, status, due).
    pub async fn update(&self, id: ObjectId, new: impl Into<CalendarObject>) -> Result<StoredObject> {
        self.update_with(id, new.into(), vec![]).await
    }

    /// [`CalendarRepo::update`] plus log entries written in the **same transaction** (`Store::put_and_log`).
    async fn update_with(&self, id: ObjectId, new: CalendarObject, extra: Vec<LogEntry>) -> Result<StoredObject> {
        let mut new = new;
        let old_stored = self.get(id).await?.ok_or(Error::NotFound(id))?;
        let old = CalendarObject::from_value(old_stored.content.clone())?;
        if old.uid() != new.uid() || old.type_name() != new.type_name() {
            return Err(Error::Conflict("uid and type of an object cannot change".into()));
        }
        let now = crate::now_utc();
        match (&old, &mut new) {
            (CalendarObject::Event(o), CalendarObject::Event(n)) => {
                n.updated = Some(now);
                n.created = n.created.take().or_else(|| o.created.clone());
                let rel = |e: &Event| (e.start.clone(), e.time_zone.clone(), e.duration.clone(), e.recurrence_rule.clone(), e.status.clone());
                if rel(o) != rel(n) && n.sequence.unwrap_or(0) <= o.sequence.unwrap_or(0) {
                    n.sequence = Some(o.sequence.unwrap_or(0) + 1);
                }
            }
            (CalendarObject::Task(o), CalendarObject::Task(n)) => {
                n.updated = Some(now);
                n.created = n.created.take().or_else(|| o.created.clone());
                if (&o.start, &o.due, &o.time_zone) != (&n.start, &n.due, &n.time_zone) && n.sequence.unwrap_or(0) <= o.sequence.unwrap_or(0) {
                    n.sequence = Some(o.sequence.unwrap_or(0) + 1);
                }
            }
            _ => unreachable!("type checked above"),
        }
        let mut o = self.build(&new, Some(&old_stored.envelope), None)?;
        // keep non-derived index fields of the original import
        if let Some(v) = old_stored.index.get("tzdb_version") {
            o.index.insert("tzdb_version".into(), v.clone());
        }
        self.store.put_and_log(o, &self.actor, extra).await?;
        self.get(id).await?.ok_or(Error::NotFound(id))
    }

    pub async fn delete(&self, id: ObjectId) -> Result<()> {
        self.get(id).await?.ok_or(Error::NotFound(id))?;
        self.store.delete(id, &self.actor).await?;
        Ok(())
    }

    // ---- queries ----------------------------------------------------------------------------

    /// All events, ordered by first start (by the store).
    pub async fn list_events(&self) -> Result<Vec<StoredObject>> {
        Ok(self.store.query(&Query::new().of_type(EVENT_TYPE).in_context(CONTEXT).ordered(Order::StartAsc)).await?)
    }

    /// All tasks, ordered by due date / end (undated last), by the store.
    pub async fn list_tasks(&self) -> Result<Vec<StoredObject>> {
        Ok(self.store.query(&Query::new().of_type(TASK_TYPE).in_context(CONTEXT).order_by_index("end_utc")).await?)
    }

    /// Event instances overlapping `[from, to)`, recurring series expanded at query time, ordered by start.
    ///
    /// The store selects the candidates with a range query on the envelope times
    /// (`start_time < to AND end_time >= from`; a series carries its last end, an unbounded one
    /// the `FOREVER` sentinel), and only those are expanded.
    pub async fn occurrences(&self, from: DateTime<Utc>, to: DateTime<Utc>) -> Result<Vec<Instance>> {
        let q = Query::new()
            .of_type(EVENT_TYPE)
            .in_context(CONTEXT)
            .time(TimeField::Start, Op::Lt, to)
            // zero-length events end at their start, so use >= on the end
            .time(TimeField::End, Op::Ge, from)
            .ordered(Order::StartAsc);
        let mut out = vec![];
        for o in self.store.query(&q).await? {
            let ev = Event::from_value(o.content.clone())?;
            for occ in recur::expand(&ev, from, to, &self.floating, usize::MAX)? {
                out.push(Instance { object: o.envelope.id, occurrence: occ });
            }
        }
        out.sort_by_key(|i| (i.occurrence.start_utc, i.object));
        Ok(out)
    }

    /// Instances starting from `now` for the next `days` days.
    pub async fn upcoming(&self, now: DateTime<Utc>, days: i64) -> Result<Vec<Instance>> {
        self.occurrences(now, now + Duration::days(days)).await
    }

    /// Tasks whose due date lies in `[from, to)` (range query on the `due_utc` index field).
    pub async fn tasks_due(&self, from: DateTime<Utc>, to: DateTime<Utc>) -> Result<Vec<StoredObject>> {
        let q = Query::new()
            .of_type(TASK_TYPE)
            .in_context(CONTEXT)
            .index_between("due_utc", fmt_utc(from), fmt_utc(to))
            .order_by_index("due_utc");
        Ok(self.store.query(&q).await?)
    }

    /// Fill `start_time` / `end_time` of objects written before fs-store 0.2 (they only have the
    /// `start_utc` / `end_utc` index fields). Returns the number of objects rewritten. Idempotent.
    pub async fn migrate_time_fields(&self) -> Result<usize> {
        let q = Query::new().in_context(CONTEXT).and(fs_store::Cond::Index { key: "start_utc".into(), op: Op::Ge, value: String::new() });
        let mut n = 0;
        for o in self.store.query(&q).await? {
            if o.envelope.start_time.is_some() || !matches!(o.envelope.object_type.as_str(), EVENT_TYPE | TASK_TYPE) {
                continue;
            }
            let obj = CalendarObject::from_value(o.content.clone())?;
            let mut fresh = self.build(&obj, Some(&o.envelope), None)?;
            // keep non-derived index fields of the original import
            if let Some(v) = o.index.get("tzdb_version") {
                fresh.index.insert("tzdb_version".into(), v.clone());
            }
            self.store.put(fresh, &self.actor).await?;
            n += 1;
        }
        Ok(n)
    }

    // ---- invitations (ADR 0020 log mapping) --------------------------------------------------

    /// Add invitees `(name, email)` (existing e-mail addresses are skipped) and log one `Invite` each,
    /// in the same transaction as the change of the participant list.
    pub async fn invite(&self, id: ObjectId, invitees: &[(&str, &str)]) -> Result<StoredObject> {
        let iri = self.get(id).await?.ok_or(Error::NotFound(id))?.envelope.iri;
        let mut obj = self.get_object(id).await?.ok_or(Error::NotFound(id))?;
        let mut extra = vec![];
        for (name, email) in invitees {
            let exists = match &obj {
                CalendarObject::Event(e) => e.participant_by_email(email).is_some(),
                CalendarObject::Task(t) => t.participant_by_email(email).is_some(),
            };
            if exists {
                continue;
            }
            let key = match &mut obj {
                CalendarObject::Event(e) => e.add_invitee(name, email),
                CalendarObject::Task(_) => return Err(Error::Conflict("only events can be invited to".into())),
            };
            extra.push(LogEntry::new(ActivityKind::Invite, json!({"participant": key, "email": email, "object": iri})));
        }
        self.update_with(id, obj, extra).await
    }

    /// Answer an invitation as participant `who` (participant key or e-mail): sets
    /// `participationStatus` (`accepted` / `tentative` / `declined`) and logs `Accept`,
    /// `TentativeAccept` or `Reject`. Participant change and log entry are one transaction
    /// (`Store::put_and_log`): a crash cannot leave the status changed without its reply.
    pub async fn respond(&self, id: ObjectId, who: &str, response: Response, comment: Option<&str>) -> Result<StoredObject> {
        let iri = self.get(id).await?.ok_or(Error::NotFound(id))?.envelope.iri;
        let mut obj = self.get_object(id).await?.ok_or(Error::NotFound(id))?;
        if let CalendarObject::Event(e) = &obj {
            if e.status.as_deref() == Some("cancelled") {
                return Err(Error::Conflict("the event is cancelled".into()));
            }
        }
        let key = {
            let ps = obj.participants();
            if ps.contains_key(who) {
                who.to_string()
            } else {
                match &obj {
                    CalendarObject::Event(e) => e.participant_by_email(who).map(str::to_string),
                    CalendarObject::Task(t) => t.participant_by_email(who).map(str::to_string),
                }
                .ok_or_else(|| Error::Conflict(format!("{who} is not a participant")))?
            }
        };
        let p = obj.participants_mut().get_mut(&key).expect("key exists");
        p.participation_status = Some(response.status().into());
        if let Some(c) = comment {
            p.participation_comment = Some(c.into());
        }
        let email = p.email.clone();
        let entry = LogEntry::new(
            response.kind(),
            json!({"participant": key, "email": email, "comment": comment, "status": response.status(), "object": iri}),
        );
        self.update_with(id, obj, vec![entry]).await
    }

    /// Invitation history (`Invite`, `Accept`, `TentativeAccept`, `Reject`) of an object, oldest first.
    ///
    /// Entries written by version 0.1 (kind `Offer` / `Accept` with the real name in `payload.type`)
    /// are still understood: a payload `type` wins over the kind.
    pub async fn invitation_log(&self, id: ObjectId) -> Result<Vec<InvitationEntry>> {
        let acts = self.store.activities(&ActivityFilter { object: Some(id), ..Default::default() }).await?;
        Ok(acts
            .into_iter()
            .filter(|a| {
                matches!(a.kind, ActivityKind::Invite | ActivityKind::Offer | ActivityKind::Accept | ActivityKind::TentativeAccept | ActivityKind::Reject)
            })
            .filter_map(|a| {
                let s = |k: &str| a.payload.get(k).and_then(Value::as_str).map(str::to_string);
                let activity = s("type").unwrap_or_else(|| a.kind.as_str().to_string());
                Some(InvitationEntry { activity, participant: s("participant")?, email: s("email"), comment: s("comment"), at: a.at })
            })
            .collect())
    }

    // ---- import / export --------------------------------------------------------------------

    /// Import JSCalendar JSON (object, array, or `Group`) in 1.0 or 2.0 form; 1.0 is upgraded first.
    pub async fn import_jscalendar(&self, json_text: &str) -> Result<ImportReport> {
        let up = upgrade(match serde_json::from_str::<Value>(json_text)? {
            Value::Array(a) => json!({"@type": "Group", "entries": a}),
            other => other,
        });
        let mut report = ImportReport { notes: up.notes, ..Default::default() };
        let entries = match up.value.get("entries") {
            Some(Value::Array(a)) => a.clone(),
            _ => vec![up.value],
        };
        for e in entries {
            let obj = match CalendarObject::from_value(e) {
                Ok(o) => o,
                Err(err) => {
                    report.failed.push(err.to_string());
                    continue;
                }
            };
            self.import_one(obj, None, &mut report).await?;
        }
        Ok(report)
    }

    /// Import an `.ics` text. Each UID gets its own original (`text/calendar`, ADR 0030), and the
    /// tz database version used is recorded in the index (`tzdb_version`).
    pub async fn import_ics(&self, text: &str) -> Result<ImportReport> {
        let mut report = ImportReport::default();
        for part in ical::split_ics(text) {
            if self.get_by_uid(&part.uid).await?.is_some() {
                report.skipped.push(part.uid);
                continue;
            }
            match ical::parse_ics(&part.text) {
                Ok((objs, notes)) => {
                    report.notes.extend(notes);
                    let original = Blob { media_type: ICS_MEDIA_TYPE.into(), bytes: part.text.into_bytes() };
                    for obj in objs {
                        self.import_one(obj, Some(original.clone()), &mut report).await?;
                    }
                }
                Err(e) => report.failed.push(format!("{}: {e}", part.uid)),
            }
        }
        Ok(report)
    }

    async fn import_one(&self, obj: CalendarObject, original: Option<Blob>, report: &mut ImportReport) -> Result<()> {
        let mut obj = obj;
        match &mut obj {
            CalendarObject::Event(e) => e.normalise(),
            CalendarObject::Task(t) => t.normalise(),
        }
        if obj.uid().is_empty() {
            report.failed.push("component without uid".into());
            return Ok(());
        }
        if self.get_by_uid(obj.uid()).await?.is_some() {
            report.skipped.push(obj.uid().to_string());
            return Ok(());
        }
        let uid = obj.uid().to_string();
        match self.create_inner(obj, original).await {
            Ok(o) => report.created.push(o.envelope.id),
            Err(e @ (Error::Invalid(_) | Error::Time(_) | Error::Recurrence(_))) => report.failed.push(format!("{uid}: {e}")),
            Err(e) => return Err(e),
        }
        Ok(())
    }

    /// Export objects as iCalendar text.
    pub async fn export_ics(&self, ids: &[ObjectId]) -> Result<String> {
        let mut objs = vec![];
        for id in ids {
            objs.push(self.get_object(*id).await?.ok_or(Error::NotFound(*id))?);
        }
        ical::to_ics(&objs)
    }
}

/// Parse a UTC timestamp for callers that bring strings (`2026-11-05T09:00:00Z`).
pub fn utc(s: &str) -> Result<DateTime<Utc>> {
    parse_utc(s)
}
