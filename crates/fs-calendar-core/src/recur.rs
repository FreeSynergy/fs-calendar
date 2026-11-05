//! Recurrence expansion at query time (JSCalendar `recurrenceRule` + `recurrenceOverrides`).
//!
//! The rule engine is the `rrule` crate (MIT OR Apache-2.0, RFC 5545 semantics, chrono-tz zones).
//! This module translates the JSCalendar rule into an RFC 5545 `RRULE`, applies the overrides
//! (exclusions, patches, extra instances) and computes UTC start/end per instance.

use std::collections::BTreeSet;

use chrono::{DateTime, Duration, NaiveDateTime, Utc};
use rrule::RRuleSet;
use serde_json::Value;

use crate::error::{Error, Result};
use crate::model::{Event, RecurrenceRule};
use crate::time::{Dur, Zone, fmt_local, parse_local, parse_utc};

/// One concrete instance of an event.
#[derive(Clone, Debug, PartialEq)]
pub struct Occurrence {
    /// `LocalDateTime` key of this instance in the master's zone (the `recurrenceOverrides` key / `recurrenceId`).
    pub recurrence_id: String,
    pub start_utc: DateTime<Utc>,
    pub end_utc: DateTime<Utc>,
    /// Instance after applying its override patch (no `recurrenceRule` / `recurrenceOverrides`; `recurrenceId` set).
    pub event: Event,
    pub overridden: bool,
}

/// Zone of an event: its `timeZone`, else `floating` (the caller's default).
pub fn zone_of(e: &Event, floating: &Zone) -> Result<Zone> {
    match e.time_zone.as_deref() {
        Some(z) => Zone::parse(z),
        None => Ok(*floating),
    }
}

fn duration_of(e: &Event) -> Result<Dur> {
    e.duration.as_deref().map(Dur::parse).transpose().map(Option::unwrap_or_default)
}

/// First start / end of the master instance in UTC.
pub fn first_span(e: &Event, floating: &Zone) -> Result<(DateTime<Utc>, DateTime<Utc>)> {
    let zone = zone_of(e, floating)?;
    let start = parse_local(e.start.as_deref().ok_or_else(|| Error::Time("event without start".into()))?)?;
    let d = duration_of(e)?;
    Ok((zone.to_utc(start), d.end_utc(&zone, start)))
}

/// JSCalendar rule -> RFC 5545 `RRULE` value (without the `RRULE:` prefix).
pub fn rrule_string(r: &RecurrenceRule, zone: &Zone) -> Result<String> {
    if r.rscale.as_deref().is_some_and(|s| !s.eq_ignore_ascii_case("gregorian")) {
        return Err(Error::Recurrence("only the gregorian rscale is supported".into()));
    }
    let mut p = vec![format!("FREQ={}", r.frequency.to_uppercase())];
    if let Some(i) = r.interval {
        p.push(format!("INTERVAL={i}"));
    }
    if let Some(c) = r.count {
        p.push(format!("COUNT={c}"));
    }
    if let Some(u) = &r.until {
        let utc = match parse_utc(u) {
            Ok(t) => t,
            Err(_) => zone.to_utc(parse_local(u)?),
        };
        p.push(format!("UNTIL={}", utc.format("%Y%m%dT%H%M%SZ")));
    }
    if !r.by_day.is_empty() {
        let days: Vec<String> = r
            .by_day
            .iter()
            .map(|d| format!("{}{}", d.nth_of_period.map(|n| n.to_string()).unwrap_or_default(), d.day.to_uppercase()))
            .collect();
        p.push(format!("BYDAY={}", days.join(",")));
    }
    let nums = |name: &str, v: &[i32], p: &mut Vec<String>| {
        if !v.is_empty() {
            p.push(format!("{name}={}", v.iter().map(i32::to_string).collect::<Vec<_>>().join(",")));
        }
    };
    nums("BYMONTHDAY", &r.by_month_day, &mut p);
    nums("BYYEARDAY", &r.by_year_day, &mut p);
    nums("BYWEEKNO", &r.by_week_no, &mut p);
    nums("BYSETPOS", &r.by_set_position, &mut p);
    if !r.by_month.is_empty() {
        let m: Vec<String> = r.by_month.iter().map(|m| m.trim_end_matches('L').to_string()).collect();
        p.push(format!("BYMONTH={}", m.join(",")));
    }
    let unums = |name: &str, v: &[u32], p: &mut Vec<String>| {
        if !v.is_empty() {
            p.push(format!("{name}={}", v.iter().map(u32::to_string).collect::<Vec<_>>().join(",")));
        }
    };
    unums("BYHOUR", &r.by_hour, &mut p);
    unums("BYMINUTE", &r.by_minute, &mut p);
    unums("BYSECOND", &r.by_second, &mut p);
    if let Some(w) = &r.first_day_of_week {
        p.push(format!("WKST={}", w.to_uppercase()));
    }
    Ok(p.join(";"))
}

/// Local start times produced by the rule itself (always includes the master start) within
/// `[after, before]` (both UTC bounds, generous: callers filter exactly).
fn rule_starts(e: &Event, zone: &Zone, after: DateTime<Utc>, before: DateTime<Utc>) -> Result<Vec<NaiveDateTime>> {
    let start = parse_local(e.start.as_deref().ok_or_else(|| Error::Time("event without start".into()))?)?;
    let mut out = BTreeSet::new();
    out.insert(start);
    let Some(rule) = &e.recurrence_rule else { return Ok(out.into_iter().collect()) };
    let rr = rrule_string(rule, zone)?;
    let dtstart = match zone {
        Zone::Utc => format!("DTSTART:{}Z", start.format("%Y%m%dT%H%M%S")),
        Zone::Iana(tz) => format!("DTSTART;TZID={}:{}", tz.name(), start.format("%Y%m%dT%H%M%S")),
    };
    let set: RRuleSet = format!("{dtstart}\nRRULE:{rr}").parse().map_err(|err| Error::Recurrence(format!("{err}")))?;
    let rz = rrule::Tz::Tz(zone.tz());
    let res = set.after(after.with_timezone(&rz)).before(before.with_timezone(&rz)).all(u16::MAX);
    out.extend(res.dates.iter().map(|d| d.naive_local()));
    Ok(out.into_iter().collect())
}

/// Apply a JSCalendar `PatchObject` (`{"title": "x", "participants/p1/name": "y", "locations/l1": null}`).
pub fn apply_patch(mut base: Value, patch: &serde_json::Map<String, Value>) -> Value {
    for (path, val) in patch {
        let parts: Vec<&str> = path.split('/').collect();
        set_path(&mut base, &parts, val.clone());
    }
    base
}

fn set_path(target: &mut Value, parts: &[&str], val: Value) {
    let Some((head, rest)) = parts.split_first() else { return };
    let head = head.replace("~1", "/").replace("~0", "~");
    if !target.is_object() {
        *target = Value::Object(Default::default());
    }
    let obj = target.as_object_mut().unwrap();
    if rest.is_empty() {
        if val.is_null() {
            obj.remove(&head);
        } else {
            obj.insert(head, val);
        }
    } else {
        set_path(obj.entry(head).or_insert(Value::Null), rest, val);
    }
}

/// All instances of `e` that overlap `[from, to)`, ordered by start, at most `limit`.
///
/// Zero-length instances count when `from <= start < to`. Instances excluded by
/// `recurrenceOverrides` are omitted; patched ones come back patched; override keys that the rule
/// does not produce are extra instances (RDATE). Floating events are interpreted in `floating`.
pub fn expand(e: &Event, from: DateTime<Utc>, to: DateTime<Utc>, floating: &Zone, limit: usize) -> Result<Vec<Occurrence>> {
    let zone = zone_of(e, floating)?;
    let d = duration_of(e)?;
    let margin = Duration::seconds(d.max_secs() + 2 * 86_400);
    let mut starts: BTreeSet<NaiveDateTime> = rule_starts(e, &zone, from - margin, to)?.into_iter().collect();
    for k in e.recurrence_overrides.keys() {
        starts.insert(parse_local(k)?); // extra instances; excluded ones are removed below
    }
    let master = Value::Object({
        let mut m = e.to_value().as_object().cloned().unwrap_or_default();
        for k in ["recurrenceRule", "recurrenceOverrides"] {
            m.remove(k);
        }
        m
    });
    let mut out = vec![];
    for local in starts {
        let key = fmt_local(local);
        let ov = e.recurrence_overrides.get(&key).and_then(Value::as_object);
        if ov.is_some_and(|o| o.get("excluded") == Some(&Value::Bool(true))) {
            continue;
        }
        let mut inst = master.clone();
        let io = inst.as_object_mut().unwrap();
        io.insert("recurrenceId".into(), Value::String(key.clone()));
        io.insert("start".into(), Value::String(key.clone()));
        if let Some(o) = ov {
            let mut patch = o.clone();
            patch.remove("excluded");
            inst = apply_patch(inst, &patch);
        }
        let ev = Event::from_value(inst)?;
        // a patched instance may move itself
        let (s_local, dur) = (ev.start.as_deref().map(parse_local).transpose()?.unwrap_or(local), duration_of(&ev)?);
        let inst_zone = zone_of(&ev, floating)?;
        let (start_utc, end_utc) = (inst_zone.to_utc(s_local), dur.end_utc(&inst_zone, s_local));
        let overlaps = if end_utc > start_utc { start_utc < to && end_utc > from } else { start_utc >= from && start_utc < to };
        if overlaps {
            out.push(Occurrence { recurrence_id: key, start_utc, end_utc, event: ev, overridden: ov.is_some() });
        }
    }
    out.sort_by_key(|o| (o.start_utc, o.recurrence_id.clone()));
    out.truncate(limit);
    Ok(out)
}

/// End of the last instance (UTC), or `None` if the series is unbounded (no `count` / `until`)
/// or the rule cannot be evaluated. Non-recurring events return their own end.
pub fn series_end(e: &Event, floating: &Zone) -> Result<Option<DateTime<Utc>>> {
    let (first_start, first_end) = first_span(e, floating)?;
    let Some(rule) = &e.recurrence_rule else {
        return Ok(Some(extra_end(e, floating, first_end)?));
    };
    if rule.count.is_none() && rule.until.is_none() {
        return Ok(None);
    }
    let far = first_start + Duration::days(366 * 400);
    let all = expand(e, first_start, far, floating, usize::MAX)?;
    Ok(all.iter().map(|o| o.end_utc).max().or(Some(first_end)))
}

fn extra_end(e: &Event, floating: &Zone, base: DateTime<Utc>) -> Result<DateTime<Utc>> {
    // overrides of a non-recurring event can still add instances
    if e.recurrence_overrides.is_empty() {
        return Ok(base);
    }
    let all = expand(e, base - Duration::days(366 * 200), base + Duration::days(366 * 200), floating, usize::MAX)?;
    Ok(all.iter().map(|o| o.end_utc).chain([base]).max().unwrap_or(base))
}
