//! Validation of Events and Tasks against JSCalendar 2.0.

use serde_json::Value;

use crate::error::{Error, Issue, Result};
use crate::model::{CalendarObject, Event, RecurrenceRule, Task, VERSION};
use crate::time::{Dur, Zone, parse_local, parse_utc};

const FREQ: [&str; 7] = ["yearly", "monthly", "weekly", "daily", "hourly", "minutely", "secondly"];
const DAYS: [&str; 7] = ["mo", "tu", "we", "th", "fr", "sa", "su"];
const PARTICIPATION: [&str; 5] = ["needs-action", "accepted", "declined", "tentative", "delegated"];

struct Ctx(Vec<Issue>);
impl Ctx {
    fn bad(&mut self, path: &str, msg: impl Into<String>) {
        self.0.push(Issue { path: path.into(), message: msg.into() });
    }
    fn one_of(&mut self, path: &str, v: &Option<String>, allowed: &[&str]) {
        if let Some(s) = v {
            if !allowed.contains(&s.as_str()) {
                self.bad(path, format!("{s:?} not in {allowed:?}"));
            }
        }
    }
}

pub fn issues(obj: &CalendarObject) -> Vec<Issue> {
    let mut c = Ctx(vec![]);
    match obj {
        CalendarObject::Event(e) => {
            common(&mut c, e.type_.as_deref(), "Event", e.version.as_deref(), &e.uid);
            if e.start.is_none() {
                c.bad("/start", "required for an Event");
            }
            c.one_of("/status", &e.status, &["confirmed", "tentative", "cancelled"]);
            if let Some(d) = &e.duration {
                if Dur::parse(d).is_err() {
                    c.bad("/duration", format!("not a duration: {d:?}"));
                }
            }
            if let Some(z) = &e.end_time_zone {
                if Zone::parse(z).is_err() {
                    c.bad("/endTimeZone", format!("not an IANA time zone: {z:?}"));
                }
            }
            shared(&mut c, e);
        }
        CalendarObject::Task(t) => {
            common(&mut c, t.type_.as_deref(), "Task", t.version.as_deref(), &t.uid);
            c.one_of("/progress", &t.progress, &["needs-action", "in-process", "completed", "failed", "cancelled"]);
            if t.percent_complete.is_some_and(|p| p > 100) {
                c.bad("/percentComplete", "0..=100");
            }
            if let Some(s) = &t.due {
                if parse_local(s).is_err() {
                    c.bad("/due", format!("not a LocalDateTime: {s:?}"));
                }
            }
            if let Some(d) = &t.estimated_duration {
                if Dur::parse(d).is_err() {
                    c.bad("/estimatedDuration", format!("not a duration: {d:?}"));
                }
            }
            if let Some(s) = &t.progress_updated {
                if parse_utc(s).is_err() {
                    c.bad("/progressUpdated", "not a UTCDateTime");
                }
            }
            shared(&mut c, t);
        }
    }
    c.0
}

pub fn validate(obj: &CalendarObject) -> Result<()> {
    let i = issues(obj);
    if i.is_empty() { Ok(()) } else { Err(Error::Invalid(i)) }
}

pub fn validate_event(e: &Event) -> Result<()> {
    validate(&CalendarObject::Event(e.clone()))
}

pub fn validate_task(t: &Task) -> Result<()> {
    validate(&CalendarObject::Task(t.clone()))
}

fn common(c: &mut Ctx, ty: Option<&str>, want: &str, version: Option<&str>, uid: &str) {
    if ty != Some(want) {
        c.bad("/@type", format!("must be {want:?}"));
    }
    if version != Some(VERSION) {
        c.bad("/version", "required, must be \"2.0\" (upgrade RFC 8984 data with `upgrade`)");
    }
    if uid.trim().is_empty() {
        c.bad("/uid", "required");
    }
}

/// Fields both types share, read through a JSON view so one function serves both.
fn shared<T: serde::Serialize>(c: &mut Ctx, obj: &T) {
    let v = serde_json::to_value(obj).unwrap_or(Value::Null);
    let s = |k: &str| v.get(k).and_then(Value::as_str);
    if let Some(st) = s("start") {
        if parse_local(st).is_err() {
            c.bad("/start", format!("not a LocalDateTime (no zone suffix, no fraction in 2.0): {st:?}"));
        }
    }
    for k in ["created", "updated"] {
        if let Some(t) = s(k) {
            if parse_utc(t).is_err() {
                c.bad(&format!("/{k}"), format!("not a UTCDateTime: {t:?}"));
            }
        }
    }
    for k in ["timeZone", "recurrenceIdTimeZone"] {
        if let Some(z) = s(k) {
            if Zone::parse(z).is_err() {
                c.bad(&format!("/{k}"), format!("not an IANA time zone (JSCalendar 2.0 has no custom zones): {z:?}"));
            }
        }
    }
    if v.get("recurrenceRules").is_some() {
        c.bad("/recurrenceRules", "obsolete in 2.0, use the single `recurrenceRule`");
    }
    if v.get("timeZones").is_some() || v.get("excludedRecurrenceRules").is_some() {
        c.bad("/timeZones", "`timeZones` and `excludedRecurrenceRules` are obsolete in 2.0");
    }
    c.one_of("/freeBusyStatus", &s("freeBusyStatus").map(String::from), &["free", "busy"]);
    c.one_of("/privacy", &s("privacy").map(String::from), &["public", "private", "secret"]);
    if let Some(r) = v.get("recurrenceRule").and_then(|r| serde_json::from_value::<RecurrenceRule>(r.clone()).ok()) {
        rule(c, &r);
        if s("start").is_none() {
            c.bad("/recurrenceRule", "needs `start`");
        }
    }
    if let Some(ov) = v.get("recurrenceOverrides").and_then(Value::as_object) {
        for k in ov.keys() {
            if parse_local(k).is_err() {
                c.bad(&format!("/recurrenceOverrides/{k}"), "key must be a LocalDateTime");
            }
        }
    }
    if let Some(ps) = v.get("participants").and_then(Value::as_object) {
        for (k, p) in ps {
            if let Some(st) = p.get("participationStatus").and_then(Value::as_str) {
                if !PARTICIPATION.contains(&st) {
                    c.bad(&format!("/participants/{k}/participationStatus"), format!("{st:?} not in {PARTICIPATION:?}"));
                }
            }
            for obsolete in ["language", "locationId"] {
                if p.get(obsolete).is_some() {
                    c.bad(&format!("/participants/{k}/{obsolete}"), "obsolete in 2.0");
                }
            }
        }
    }
}

fn rule(c: &mut Ctx, r: &RecurrenceRule) {
    if !FREQ.contains(&r.frequency.as_str()) {
        c.bad("/recurrenceRule/frequency", format!("unknown frequency {:?}", r.frequency));
    }
    if r.interval == Some(0) {
        c.bad("/recurrenceRule/interval", "must be >= 1");
    }
    if r.count.is_some() && r.until.is_some() {
        c.bad("/recurrenceRule", "`count` and `until` are mutually exclusive");
    }
    if r.count == Some(0) {
        c.bad("/recurrenceRule/count", "must be >= 1");
    }
    if let Some(u) = &r.until {
        if parse_local(u).is_err() && parse_utc(u).is_err() {
            c.bad("/recurrenceRule/until", format!("not a date-time: {u:?}"));
        }
    }
    for d in &r.by_day {
        if !DAYS.contains(&d.day.as_str()) {
            c.bad("/recurrenceRule/byDay", format!("unknown day {:?}", d.day));
        }
    }
    if r.rscale.as_deref().is_some_and(|s| !s.eq_ignore_ascii_case("gregorian")) {
        c.bad("/recurrenceRule/rscale", "only gregorian is supported");
    }
}
