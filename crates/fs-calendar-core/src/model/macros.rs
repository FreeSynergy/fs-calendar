//! Macros that fix the skeleton of every JSCalendar object.
//!
//! Pattern: Template method (macro form) — `typed_object!` adds `@type` and the flattened `extra`
//! map (lossless round trips); `calendar_object!` adds the JSCalendar members and common methods of an object, so
//! `Event` only lists what is its own.

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
            /// The JSCalendar `@type` of this object.
            pub const TYPE: &'static str = $tag;
            /// Set `@type` and `version` if missing.
            pub fn normalise(&mut self) {
                self.type_.get_or_insert_with(|| $tag.to_string());
                self.version.get_or_insert_with(|| VERSION.to_string());
            }
            /// Vendor / unknown property.
            pub fn vendor(&self, key: &str) -> Option<&Value> {
                self.extra.get(key)
            }
            /// Set a vendor / unknown property.
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
            /// The object as a JSON value (the stored content).
            ///
            /// # Panics
            /// Never in practice: the object holds only JSON-compatible data.
            pub fn to_value(&self) -> Value {
                serde_json::to_value(self).expect("serialises")
            }
            /// Read an object from JSON.
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
