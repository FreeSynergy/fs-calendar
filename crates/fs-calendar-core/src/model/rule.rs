//! The recurrence rule of JSCalendar 2.0.
//!
//! Pattern: Value object — plain serde data; the rule *engine* is a separate strategy (`recur`).

// The members mirror the JSCalendar property names one to one; documenting each would only
// repeat the specification.
#![allow(missing_docs)]

use serde::{Deserialize, Serialize};

use super::consts::Extra;

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
