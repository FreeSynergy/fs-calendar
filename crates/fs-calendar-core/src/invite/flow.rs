//! Inviting and answering: participant change and log entry in one transaction.
//!
//! Pattern: State (client) — `respond` moves the participant through
//! [`ParticipationStatus::respond`] and writes the matching `Accept` / `TentativeAccept` /
//! `Reject` activity with `put_and_log`, so a crash cannot leave the status changed without its reply.

use fs_store::{ActivityKind, LogEntry, ObjectId, StoredObject};
use serde_json::json;

use super::status::{ParticipationStatus, Response};
use crate::error::{Error, Result};
use crate::model::Event;
use crate::repo::CalendarRepo;

impl CalendarRepo {
    /// Add invitees `(name, email)` (existing e-mail addresses are skipped) and log one `Invite`
    /// each, in the same transaction as the change of the participant list.
    pub async fn invite(&self, id: ObjectId, invitees: &[(&str, &str)]) -> Result<StoredObject> {
        let _one_at_a_time = self.writes.lock().await;
        let iri = self.get(id).await?.ok_or(Error::NotFound(id))?.envelope.iri;
        let mut obj = self.get_event(id).await?.ok_or(Error::NotFound(id))?;
        let mut extra = vec![];
        for (name, email) in invitees {
            if obj.participant_by_email(email).is_some() {
                continue;
            }
            let key = obj.add_invitee(name, email);
            let payload = json!({"participant": key, "email": email, "object": iri});
            extra.push(LogEntry::new(ActivityKind::Invite, payload));
        }
        self.update_with(id, obj, extra).await
    }

    /// Answer an invitation as participant `who` (participant key or e-mail): sets
    /// `participationStatus` (`accepted` / `tentative` / `declined`) and logs `Accept`,
    /// `TentativeAccept` or `Reject`. Participant change and log entry are one transaction
    /// (`put_and_log`): a crash cannot leave the status changed without its reply.
    pub async fn respond(
        &self,
        id: ObjectId,
        who: &str,
        response: Response,
        comment: Option<&str>,
    ) -> Result<StoredObject> {
        let _one_at_a_time = self.writes.lock().await;
        let iri = self.get(id).await?.ok_or(Error::NotFound(id))?.envelope.iri;
        let mut obj = self.get_event(id).await?.ok_or(Error::NotFound(id))?;
        if obj.status.as_deref() == Some("cancelled") {
            return Err(Error::Conflict("the event is cancelled".into()));
        }
        let key = participant_key(&obj, who)?;
        let p = obj
            .participants
            .get_mut(&key)
            .ok_or_else(|| Error::Conflict(format!("{who} is not a participant")))?;
        let current = p
            .participation_status
            .as_deref()
            .and_then(ParticipationStatus::parse)
            .unwrap_or(ParticipationStatus::INITIAL);
        let next = current.respond(response);
        p.participation_status = Some(next.as_str().into());
        if let Some(c) = comment {
            p.participation_comment = Some(c.into());
        }
        let email = p.email.clone();
        let entry = LogEntry::new(
            response.kind(),
            json!({"participant": key, "email": email, "comment": comment, "status": next.as_str(), "object": iri}),
        );
        self.update_with(id, obj, vec![entry]).await
    }
}

/// The participant key for `who` (a key or an e-mail address).
fn participant_key(obj: &Event, who: &str) -> Result<String> {
    if obj.participants.contains_key(who) {
        return Ok(who.to_string());
    }
    obj.participant_by_email(who)
        .map(str::to_string)
        .ok_or_else(|| Error::Conflict(format!("{who} is not a participant")))
}
