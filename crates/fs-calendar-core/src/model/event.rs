//! The typed `Event`.
//!
//! Pattern: Template method (macro form) — `calendar_object!` of `fs-jscalendar` supplies the shared members; this
//! file adds what is specific (`duration` / `status`) and the constructors. (`Task` belongs to `fs-tasks`, ADR 0050.)

// The members mirror the JSCalendar property names one to one; documenting each would only
// repeat the specification.
#![allow(missing_docs)]

use fs_jscalendar::model::{Participant, next_participant_key};

use crate::now_utc;

fs_jscalendar::calendar_object!(
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

impl Event {
    /// A new event: fresh `uid` (`urn:uuid:` UUIDv7), `start` local in `time_zone`, `duration`.
    pub fn new(title: impl Into<String>, start: &str, time_zone: &str, duration: &str) -> Self {
        let now = now_utc();
        let mut e = Event {
            uid: fs_jscalendar::new_uid(),
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
    pub fn add_invitee(&mut self, name: &str, email: &str) -> String {
        let key = next_participant_key(&self.participants);
        let mut invitee = Participant::person(name, email, "attendee");
        invitee.expect_reply = Some(true);
        self.participants.insert(key.clone(), invitee);
        key
    }
}
