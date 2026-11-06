//! The invitation history of an object, read from the activity log.
//!
//! Pattern: Observer-style read model — the log is the source of truth; this view filters the
//! invitation activities and understands the 0.1 encoding.

use chrono::{DateTime, Utc};
use fs_store::{ActivityFilter, ActivityKind, ObjectId};
use serde_json::Value;

use crate::error::Result;
use crate::repo::CalendarRepo;

/// One entry of the invitation history of an object.
#[derive(Clone, Debug, PartialEq)]
pub struct InvitationEntry {
    /// `Invite`, `Accept`, `TentativeAccept` or `Reject`.
    pub activity: String,
    /// Participant key.
    pub participant: String,
    /// E-mail address of the participant, if known.
    pub email: Option<String>,
    /// The participant's comment, if any.
    pub comment: Option<String>,
    /// When it was logged.
    pub at: DateTime<Utc>,
}

impl CalendarRepo {
    /// Invitation history (`Invite`, `Accept`, `TentativeAccept`, `Reject`) of an object, oldest first.
    ///
    /// Entries written by version 0.1 (kind `Offer` / `Accept` with the real name in
    /// `payload.type`) are still understood: a payload `type` wins over the kind.
    pub async fn invitation_log(&self, id: ObjectId) -> Result<Vec<InvitationEntry>> {
        let filter = ActivityFilter {
            object: Some(id),
            ..Default::default()
        };
        let acts = self.log.activities(&filter).await?;
        Ok(acts
            .into_iter()
            .filter(|a| {
                matches!(
                    a.kind,
                    ActivityKind::Invite
                        | ActivityKind::Offer
                        | ActivityKind::Accept
                        | ActivityKind::TentativeAccept
                        | ActivityKind::Reject
                )
            })
            .filter_map(|a| {
                let s = |k: &str| a.payload.get(k).and_then(Value::as_str).map(str::to_string);
                let activity = s("type").unwrap_or_else(|| a.kind.as_str().to_string());
                Some(InvitationEntry {
                    activity,
                    participant: s("participant")?,
                    email: s("email"),
                    comment: s("comment"),
                    at: a.at,
                })
            })
            .collect())
    }
}
