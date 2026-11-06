//! A JSCalendar `Participant`.
//!
//! Pattern: Value object — plain serde data; the invitation *flow* lives in `invite`.

// The members mirror the JSCalendar property names one to one; documenting each would only
// repeat the specification.
#![allow(missing_docs)]

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::consts::Extra;

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
