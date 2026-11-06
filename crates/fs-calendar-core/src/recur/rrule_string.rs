//! JSCalendar recurrence rule to an RFC 5545 `RRULE` value.
//!
//! Pattern: Adapter — translates our typed rule into the text form `rrule` parses. `UNTIL` is
//! written in wall-clock terms (see `rrule_engine`).

use crate::error::{Error, Result};
use crate::model::RecurrenceRule;
use crate::time::{Zone, parse_local, parse_utc};

/// JSCalendar rule -> RFC 5545 `RRULE` value (without the `RRULE:` prefix).
pub fn rrule_string(r: &RecurrenceRule, zone: &Zone) -> Result<String> {
    if r.rscale
        .as_deref()
        .is_some_and(|s| !s.eq_ignore_ascii_case("gregorian"))
    {
        return Err(Error::Recurrence(
            "only the gregorian rscale is supported".into(),
        ));
    }
    let mut p = vec![format!("FREQ={}", r.frequency.to_uppercase())];
    if let Some(i) = r.interval {
        p.push(format!("INTERVAL={i}"));
    }
    if let Some(c) = r.count {
        p.push(format!("COUNT={c}"));
    }
    if let Some(u) = &r.until {
        // Wall-clock semantics: `until` is compared with local start times (see `rule_starts`).
        let local = match parse_utc(u) {
            Ok(t) => zone.to_local(t),
            Err(_) => parse_local(u)?,
        };
        p.push(format!("UNTIL={}Z", local.format("%Y%m%dT%H%M%S")));
    }
    if !r.by_day.is_empty() {
        let days: Vec<String> = r
            .by_day
            .iter()
            .map(|d| {
                format!(
                    "{}{}",
                    d.nth_of_period.map(|n| n.to_string()).unwrap_or_default(),
                    d.day.to_uppercase()
                )
            })
            .collect();
        p.push(format!("BYDAY={}", days.join(",")));
    }
    let nums = |name: &str, v: &[i32], p: &mut Vec<String>| {
        if !v.is_empty() {
            p.push(format!(
                "{name}={}",
                v.iter().map(i32::to_string).collect::<Vec<_>>().join(",")
            ));
        }
    };
    nums("BYMONTHDAY", &r.by_month_day, &mut p);
    nums("BYYEARDAY", &r.by_year_day, &mut p);
    nums("BYWEEKNO", &r.by_week_no, &mut p);
    nums("BYSETPOS", &r.by_set_position, &mut p);
    if !r.by_month.is_empty() {
        let m: Vec<String> = r
            .by_month
            .iter()
            .map(|m| m.trim_end_matches('L').to_string())
            .collect();
        p.push(format!("BYMONTH={}", m.join(",")));
    }
    let unums = |name: &str, v: &[u32], p: &mut Vec<String>| {
        if !v.is_empty() {
            p.push(format!(
                "{name}={}",
                v.iter().map(u32::to_string).collect::<Vec<_>>().join(",")
            ));
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
