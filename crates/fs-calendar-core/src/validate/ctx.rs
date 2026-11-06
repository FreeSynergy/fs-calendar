//! Collector for validation problems and the allowed value lists.
//!
//! Pattern: Collecting parameter — every rule reports into one `Ctx`, so an object is checked
//! completely and the caller sees all problems at once.

use crate::error::Issue;

pub(super) const FREQ: [&str; 7] = [
    "yearly", "monthly", "weekly", "daily", "hourly", "minutely", "secondly",
];
pub(super) const DAYS: [&str; 7] = ["mo", "tu", "we", "th", "fr", "sa", "su"];

pub(super) struct Ctx(pub(super) Vec<Issue>);
impl Ctx {
    pub(super) fn bad(&mut self, path: &str, msg: impl Into<String>) {
        self.0.push(Issue {
            path: path.into(),
            message: msg.into(),
        });
    }
    pub(super) fn one_of(&mut self, path: &str, v: Option<&str>, allowed: &[&str]) {
        if let Some(s) = v {
            if !allowed.contains(&s) {
                self.bad(path, format!("{s:?} not in {allowed:?}"));
            }
        }
    }
}
