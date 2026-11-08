//! A window is walked from its own start only where that gives the same instances as walking from the
//! series start: compared with the engine from the real start, over many rules and windows.

use chrono::{Duration, NaiveDate, NaiveDateTime};
use fs_calendar_core::recur::expand;
use fs_calendar_core::time::Zone;
use fs_calendar_core::*;
use serde_json::{Value, json};

/// A fixed-seed generator: the same cases every run.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self, below: u64) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 33) % below
    }

    fn small(&mut self, below: u32) -> u32 {
        u32::try_from(self.next(u64::from(below))).unwrap()
    }

    fn index(&mut self, below: usize) -> usize {
        usize::try_from(self.next(below as u64)).unwrap()
    }

    fn seconds(&mut self, below: u64) -> i64 {
        i64::try_from(self.next(below)).unwrap()
    }

    fn some_of<T: Clone>(&mut self, all: &[T], at_most: usize) -> Vec<T> {
        let count = 1 + self.index(at_most);
        (0..count)
            .map(|_| all[self.index(all.len())].clone())
            .collect()
    }
}

fn event(start: NaiveDateTime, rule: &Value) -> Event {
    let mut e = Event::new(
        "Rule",
        &start.format("%Y-%m-%dT%H:%M:%S").to_string(),
        "Etc/UTC",
        "PT1S",
    );
    e.recurrence_rule = Some(serde_json::from_value(rule.clone()).unwrap());
    e
}

/// The starts in `[from, to)` by the engine from the real start, and the real start itself.
fn reference(e: &Event, from: NaiveDateTime, to: NaiveDateTime) -> Vec<NaiveDateTime> {
    let rule = e.recurrence_rule.as_ref().unwrap();
    let start = fs_calendar_core::time::parse_local(e.start.as_deref().unwrap()).unwrap();
    let all = RRuleEngine
        .starts(rule, start, &Zone::Utc, from.and_utc(), to.and_utc())
        .unwrap();
    let mut got: Vec<_> = all
        .into_iter()
        .chain([start])
        .filter(|s| *s >= from && *s < to)
        .collect();
    got.sort();
    got.dedup();
    got
}

fn walked(e: &Event, from: NaiveDateTime, to: NaiveDateTime) -> Vec<NaiveDateTime> {
    let found = expand(e, from.and_utc(), to.and_utc(), &Zone::Utc, 100_000).unwrap();
    let mut got: Vec<_> = found.iter().map(|o| o.start_utc.naive_utc()).collect();
    got.sort();
    got.dedup();
    got
}

fn day(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).unwrap()
}

#[test]
fn the_two_reported_rules_give_the_same_instances_from_any_window() {
    // weekly Monday or Friday, the first of the set, from a Wednesday: only Mondays
    let start = day(2020, 1, 8).and_hms_opt(8, 0, 0).unwrap();
    let rule = json!({"frequency": "weekly", "byDay": [{"day": "mo"}, {"day": "fr"}], "bySetPosition": [1]});
    let e = event(start, &rule);
    let (from, to) = (
        day(2027, 1, 1).and_hms_opt(0, 0, 0).unwrap(),
        day(2027, 1, 20).and_hms_opt(0, 0, 0).unwrap(),
    );
    let got = walked(&e, from, to);
    assert!(
        !got.is_empty() && got.iter().all(|s| s.format("%a").to_string() == "Mon"),
        "{got:?}"
    );

    // minutely every seven minutes in hours 2 and 3: the phase does not depend on the window
    let start = day(2026, 1, 1).and_hms_opt(0, 0, 0).unwrap();
    let rule = json!({"frequency": "minutely", "interval": 7, "byHour": [2, 3]});
    let e = event(start, &rule);
    let at = |h, m| day(2026, 3, 2).and_hms_opt(h, m, 0).unwrap();
    let from_midnight = walked(&e, at(0, 0), at(4, 0));
    let from_half_past_two = walked(&e, at(2, 30), at(4, 0));
    let tail: Vec<_> = from_midnight
        .iter()
        .copied()
        .filter(|s| *s >= at(2, 30))
        .collect();
    assert_eq!(from_half_past_two, tail);
    assert_eq!(from_midnight, reference(&e, at(0, 0), at(4, 0)));
}

#[test]
fn many_rules_and_windows_give_what_the_engine_gives_from_the_real_start() {
    let mut rng = Lcg(20_261_108);
    let frequencies = [
        "secondly", "minutely", "hourly", "daily", "weekly", "monthly", "yearly",
    ];
    let days = ["mo", "tu", "we", "th", "fr", "sa", "su"].map(|d| json!({"day": d}));
    let mut checked = 0;
    for case in 0..400 {
        let frequency = frequencies[rng.index(frequencies.len())];
        let mut rule = json!({"frequency": frequency});
        if rng.next(2) == 0 {
            rule["interval"] = json!(1 + rng.next(9));
        }
        if rng.next(3) == 0 {
            rule["byDay"] = json!(rng.some_of(&days, 3));
        }
        if rng.next(3) == 0 {
            rule["byHour"] = json!(rng.some_of(&(0..24).collect::<Vec<u64>>(), 3));
        }
        if rng.next(4) == 0 {
            rule["byMinute"] = json!(rng.some_of(&(0..60).collect::<Vec<u64>>(), 3));
        }
        if rng.next(4) == 0 {
            rule["byMonth"] =
                json!(rng.some_of(&(1..=12).map(|m| m.to_string()).collect::<Vec<_>>(), 3));
        }
        if rng.next(4) == 0
            && rule
                .as_object()
                .unwrap()
                .keys()
                .any(|k| k.starts_with("by"))
        {
            rule["bySetPosition"] = json!([1]);
        }
        if rng.next(4) == 0 {
            rule["until"] = json!(format!("2027-0{}-15T00:00:00", 1 + rng.next(9)));
        }
        let start = day(2026, 1 + rng.small(12), 1 + rng.small(28))
            .and_hms_opt(rng.small(24), rng.small(60), rng.small(60))
            .unwrap();
        // how far from the start a window can be and still be cheap for the reference
        let (reach, width) = match frequency {
            "secondly" => (2, 60 * 60),
            "minutely" => (20, 24 * 3600),
            "hourly" => (200, 2 * 24 * 3600),
            _ => (500, 14 * 24 * 3600),
        };
        let from = start + Duration::seconds(rng.seconds(reach * 86_400));
        let to = from + Duration::seconds(1 + rng.seconds(width));
        let e = event(start, &rule);
        assert_eq!(
            walked(&e, from, to),
            reference(&e, from, to),
            "case {case}: {rule} from {start} window {from} to {to}"
        );
        checked += 1;
    }
    assert_eq!(checked, 400);
}
