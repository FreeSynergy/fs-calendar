//! The typed `Event` and `Task`.
//!
//! Pattern: Template method (macro form) — `calendar_object!` supplies the shared members; this
//! file adds what is specific (`duration` / `status` for events, `due` / `progress` for tasks) and the constructors.

// The members mirror the JSCalendar property names one to one; documenting each would only
// repeat the specification.
#![allow(missing_docs)]

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::consts::{Extra, Map, VERSION, is_empty_map};
use super::participant::Participant;
use super::rule::RecurrenceRule;
use crate::clock::now_utc;
use crate::error::Result;

calendar_object!(
    /// A JSCalendar `Event`.
    Event, "Event" {
        #[serde(skip_serializing_if = "Option::is_none")]
        pub duration: Option<String>,
        #[serde(rename = "endTimeZone", skip_serializing_if = "Option::is_none")]
        pub end_time_zone: Option<String>,
        /// `confirmed` / `tentative` / `cancelled`
        #[serde(skip_serializing_if = "Option::is_none")]
        pub status: Option<String>,
    }
);

calendar_object!(
    /// A JSCalendar `Task` (also used by projects).
    Task, "Task" {
        #[serde(skip_serializing_if = "Option::is_none")]
        pub due: Option<String>,
        #[serde(rename = "estimatedDuration", skip_serializing_if = "Option::is_none")]
        pub estimated_duration: Option<String>,
        #[serde(rename = "percentComplete", skip_serializing_if = "Option::is_none")]
        pub percent_complete: Option<u8>,
        /// `needs-action` / `in-process` / `completed` / `failed` / `cancelled`
        #[serde(skip_serializing_if = "Option::is_none")]
        pub progress: Option<String>,
        #[serde(rename = "progressUpdated", skip_serializing_if = "Option::is_none")]
        pub progress_updated: Option<String>,
    }
);

impl Event {
    /// A new event: fresh `uid` (`urn:uuid:` UUIDv7), `start` local in `time_zone`, `duration`.
    pub fn new(title: impl Into<String>, start: &str, time_zone: &str, duration: &str) -> Self {
        let now = now_utc();
        let mut e = Event {
            uid: format!("urn:uuid:{}", fs_store::ObjectId::new()),
            title: Some(title.into()),
            start: Some(start.into()),
            time_zone: Some(time_zone.into()),
            duration: Some(duration.into()),
            created: Some(now.clone()),
            updated: Some(now),
            ..Default::default()
        };
        e.normalise();
        e
    }

    /// Add an invitee (status `needs-action`, `expectReply`); returns the participant key.
    ///
    /// # Panics
    /// Never: among `n + 1` candidate keys at most `n` are taken.
    pub fn add_invitee(&mut self, name: &str, email: &str) -> String {
        let key = (1..=self.participants.len() + 1)
            .map(|i| format!("p{i}"))
            .find(|k| !self.participants.contains_key(k))
            .expect("one of n + 1 candidate keys is free");
        self.participants.insert(
            key.clone(),
            Participant {
                type_: Some("Participant".into()),
                name: Some(name.into()),
                email: Some(email.into()),
                calendar_address: Some(format!("mailto:{email}")),
                kind: Some("individual".into()),
                roles: BTreeMap::from([("attendee".to_string(), true)]),
                participation_status: Some("needs-action".into()),
                expect_reply: Some(true),
                ..Default::default()
            },
        );
        key
    }
}

impl Task {
    pub fn new(title: impl Into<String>) -> Self {
        let now = now_utc();
        let mut t = Task {
            uid: format!("urn:uuid:{}", fs_store::ObjectId::new()),
            title: Some(title.into()),
            created: Some(now.clone()),
            updated: Some(now),
            progress: Some("needs-action".into()),
            ..Default::default()
        };
        t.normalise();
        t
    }
}
