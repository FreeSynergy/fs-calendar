//! Typed JSCalendar 2.0 (`draft-ietf-calext-jscalendarbis`) `Event` and `Task`.
//!
//! Unknown and vendor properties (`<domain>:<name>`) stay verbatim in `extra`, so a round trip
//! loses nothing. Properties we do not model (`locations`, `virtualLocations`, `links`, `alerts`,
//! `recurrenceOverrides` patches' content, reserved iTIP properties such as `sendTo`) also live
//! there or in `serde_json::Value` fields.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{Error, Result};

pub type Map<T> = BTreeMap<String, T>;
pub type Extra = BTreeMap<String, Value>;

pub const VERSION: &str = "2.0";
pub const VENDOR: &str = "freesynergy.net:";
/// Where obsolete 1.0 properties go when upgrading (see [`crate::upgrade`]).
pub const LEGACY_KEY: &str = "freesynergy.net:legacy";
/// RFC 10051 task status has no JSCalendar form yet.
pub const TASK_STATUS_KEY: &str = "freesynergy.net:taskStatus";

fn is_empty_map<T>(m: &Map<T>) -> bool {
    m.is_empty()
}

macro_rules! typed_object {
    ($(#[$m:meta])* $name:ident { $($body:tt)* }) => {
        $(#[$m])*
        #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
        #[serde(default)]
        pub struct $name {
            #[serde(rename = "@type", skip_serializing_if = "Option::is_none")]
            pub type_: Option<String>,
            $($body)*
            #[serde(flatten)]
            pub extra: Extra,
        }
    };
}

typed_object!(
    /// `NDay` of a recurrence rule.
    NDay {
        pub day: String,
        #[serde(rename = "nthOfPeriod", skip_serializing_if = "Option::is_none")]
        pub nth_of_period: Option<i32>,
    }
);

typed_object!(
    /// The single `recurrenceRule` of JSCalendar 2.0.
    RecurrenceRule {
        pub frequency: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub interval: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub rscale: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub skip: Option<String>,
        #[serde(rename = "firstDayOfWeek", skip_serializing_if = "Option::is_none")]
        pub first_day_of_week: Option<String>,
        #[serde(rename = "byDay", skip_serializing_if = "Vec::is_empty")]
        pub by_day: Vec<NDay>,
        #[serde(rename = "byMonthDay", skip_serializing_if = "Vec::is_empty")]
        pub by_month_day: Vec<i32>,
        #[serde(rename = "byMonth", skip_serializing_if = "Vec::is_empty")]
        pub by_month: Vec<String>,
        #[serde(rename = "byYearDay", skip_serializing_if = "Vec::is_empty")]
        pub by_year_day: Vec<i32>,
        #[serde(rename = "byWeekNo", skip_serializing_if = "Vec::is_empty")]
        pub by_week_no: Vec<i32>,
        #[serde(rename = "byHour", skip_serializing_if = "Vec::is_empty")]
        pub by_hour: Vec<u32>,
        #[serde(rename = "byMinute", skip_serializing_if = "Vec::is_empty")]
        pub by_minute: Vec<u32>,
        #[serde(rename = "bySecond", skip_serializing_if = "Vec::is_empty")]
        pub by_second: Vec<u32>,
        #[serde(rename = "bySetPosition", skip_serializing_if = "Vec::is_empty")]
        pub by_set_position: Vec<i32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub count: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub until: Option<String>,
    }
);

typed_object!(
    Participant {
        #[serde(skip_serializing_if = "Option::is_none")]
        pub name: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub email: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub kind: Option<String>,
        #[serde(skip_serializing_if = "BTreeMap::is_empty")]
        pub roles: BTreeMap<String, bool>,
        #[serde(rename = "participationStatus", skip_serializing_if = "Option::is_none")]
        pub participation_status: Option<String>,
        #[serde(rename = "participationComment", skip_serializing_if = "Option::is_none")]
        pub participation_comment: Option<String>,
        #[serde(rename = "expectReply", skip_serializing_if = "Option::is_none")]
        pub expect_reply: Option<bool>,
        #[serde(rename = "calendarAddress", skip_serializing_if = "Option::is_none")]
        pub calendar_address: Option<String>,
    }
);

macro_rules! calendar_object {
    ($(#[$m:meta])* $name:ident, $tag:literal { $($body:tt)* }) => {
        typed_object!(
            $(#[$m])*
            $name {
                #[serde(skip_serializing_if = "Option::is_none")]
                pub version: Option<String>,
                pub uid: String,
                #[serde(rename = "prodId", skip_serializing_if = "Option::is_none")]
                pub prod_id: Option<String>,
                #[serde(skip_serializing_if = "Option::is_none")]
                pub created: Option<String>,
                #[serde(skip_serializing_if = "Option::is_none")]
                pub updated: Option<String>,
                #[serde(skip_serializing_if = "Option::is_none")]
                pub sequence: Option<u32>,
                #[serde(skip_serializing_if = "Option::is_none")]
                pub title: Option<String>,
                #[serde(skip_serializing_if = "Option::is_none")]
                pub description: Option<String>,
                #[serde(skip_serializing_if = "Option::is_none")]
                pub locale: Option<String>,
                /// `LocalDateTime`, interpreted in `timeZone` (floating if absent).
                #[serde(skip_serializing_if = "Option::is_none")]
                pub start: Option<String>,
                #[serde(rename = "timeZone", skip_serializing_if = "Option::is_none")]
                pub time_zone: Option<String>,
                #[serde(rename = "showWithoutTime", skip_serializing_if = "Option::is_none")]
                pub show_without_time: Option<bool>,
                #[serde(rename = "freeBusyStatus", skip_serializing_if = "Option::is_none")]
                pub free_busy_status: Option<String>,
                #[serde(skip_serializing_if = "Option::is_none")]
                pub privacy: Option<String>,
                #[serde(skip_serializing_if = "Option::is_none")]
                pub priority: Option<i32>,
                #[serde(rename = "recurrenceId", skip_serializing_if = "Option::is_none")]
                pub recurrence_id: Option<String>,
                #[serde(rename = "recurrenceIdTimeZone", skip_serializing_if = "Option::is_none")]
                pub recurrence_id_time_zone: Option<String>,
                #[serde(rename = "recurrenceRule", skip_serializing_if = "Option::is_none")]
                pub recurrence_rule: Option<RecurrenceRule>,
                /// Key: `LocalDateTime` of the instance; value: patch object (`{"excluded":true}`, `{"title":"..."}`, `{"participants/p1/participationStatus":"declined"}`).
                #[serde(rename = "recurrenceOverrides", skip_serializing_if = "is_empty_map")]
                pub recurrence_overrides: Map<Value>,
                #[serde(skip_serializing_if = "is_empty_map")]
                pub participants: Map<Participant>,
                #[serde(rename = "relatedTo", skip_serializing_if = "is_empty_map")]
                pub related_to: Map<Value>,
                #[serde(skip_serializing_if = "is_empty_map")]
                pub keywords: Map<bool>,
                $($body)*
            }
        );
        impl $name {
            pub const TYPE: &'static str = $tag;
            /// Set `@type` and `version` if missing.
            pub fn normalise(&mut self) {
                self.type_.get_or_insert_with(|| $tag.to_string());
                self.version.get_or_insert_with(|| VERSION.to_string());
            }
            pub fn vendor(&self, key: &str) -> Option<&Value> {
                self.extra.get(key)
            }
            pub fn set_vendor(&mut self, key: &str, v: Value) {
                self.extra.insert(key.to_string(), v);
            }
            /// `title`, or a placeholder (the envelope `name` may never be empty, ADR 0032).
            pub fn display_title(&self) -> String {
                match self.title.as_deref().map(str::trim) {
                    Some(t) if !t.is_empty() => t.to_string(),
                    _ => format!("(untitled {})", $tag.to_lowercase()),
                }
            }
            pub fn to_value(&self) -> Value {
                serde_json::to_value(self).expect("serialises")
            }
            pub fn from_value(v: Value) -> Result<Self> {
                Ok(serde_json::from_value(v)?)
            }
            /// Key of the participant with this e-mail address (case-insensitive).
            pub fn participant_by_email(&self, email: &str) -> Option<&str> {
                let want = email.trim().trim_start_matches("mailto:").to_lowercase();
                self.participants.iter().find_map(|(k, p)| {
                    let e = p.email.as_deref().or(p.calendar_address.as_deref())?;
                    (e.trim().trim_start_matches("mailto:").to_lowercase() == want).then_some(k.as_str())
                })
            }
        }
    };
}

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
        let now = crate::now_utc();
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
    pub fn add_invitee(&mut self, name: &str, email: &str) -> String {
        let key = (1..).map(|i| format!("p{i}")).find(|k| !self.participants.contains_key(k)).unwrap();
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
        let now = crate::now_utc();
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

/// Either JSCalendar component we manage.
#[derive(Clone, Debug, PartialEq)]
pub enum CalendarObject {
    Event(Event),
    Task(Task),
}

impl CalendarObject {
    /// Dispatch on `@type` (`Event` / `Task`).
    pub fn from_value(v: Value) -> Result<Self> {
        match v.get("@type").and_then(Value::as_str) {
            Some("Event") => Ok(Self::Event(Event::from_value(v)?)),
            Some("Task") => Ok(Self::Task(Task::from_value(v)?)),
            other => Err(Error::Invalid(vec![crate::Issue {
                path: "/@type".into(),
                message: format!("expected \"Event\" or \"Task\", got {other:?}"),
            }])),
        }
    }
    pub fn to_value(&self) -> Value {
        match self {
            Self::Event(e) => e.to_value(),
            Self::Task(t) => t.to_value(),
        }
    }
    pub fn uid(&self) -> &str {
        match self {
            Self::Event(e) => &e.uid,
            Self::Task(t) => &t.uid,
        }
    }
    pub fn title(&self) -> String {
        match self {
            Self::Event(e) => e.display_title(),
            Self::Task(t) => t.display_title(),
        }
    }
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Event(_) => "Event",
            Self::Task(_) => "Task",
        }
    }
    pub fn participants_mut(&mut self) -> &mut Map<Participant> {
        match self {
            Self::Event(e) => &mut e.participants,
            Self::Task(t) => &mut t.participants,
        }
    }
    pub fn participants(&self) -> &Map<Participant> {
        match self {
            Self::Event(e) => &e.participants,
            Self::Task(t) => &t.participants,
        }
    }
}

impl From<Event> for CalendarObject {
    fn from(e: Event) -> Self {
        Self::Event(e)
    }
}
impl From<Task> for CalendarObject {
    fn from(t: Task) -> Self {
        Self::Task(t)
    }
}
