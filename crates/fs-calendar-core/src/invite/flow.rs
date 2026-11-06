//! Inviting and answering: participant change and log entry in one transaction.
//!
//! Pattern: State (client) — `respond` moves the participant through
//! [`ParticipationStatus::respond`] and writes the matching `Accept` / `TentativeAccept` /
//! `Reject` activity with `put_and_log`, so a crash cannot leave the status changed without its reply.

use fs_store::{ActivityKind, LogEntry, ObjectId, StoredObject};
use serde_json::json;

use super::status::{ParticipationStatus, Response};
use crate::error::{Error, Result};
use crate::model::CalendarObject;
use crate::repo::CalendarRepo;

impl CalendarRepo {
    /// Add invitees `(name, email)` (existing e-mail addresses are skipped) and log one `Invite`
    /// each, in the same transaction as the change of the participant list.
    pub async fn invite(&self, id: ObjectId, invitees: &[(&str, &str)]) -> Result<StoredObject> {
        let iri = self.get(id).await?.ok_or(Error::NotFound(id))?.envelope.iri;
        let mut obj = self.get_object(id).await?.ok_or(Error::NotFound(id))?;
        let mut extra = vec![];
        for (name, email) in invitees {
            let exists = match &obj {
                CalendarObject::Event(e) => e.participant_by_email(email).is_some(),
                CalendarObject::Task(t) => t.participant_by_email(email).is_some(),
            };
            if exists {
                continue;
            }
            let key = match &mut obj {
                CalendarObject::Event(e) => e.add_invitee(name, email),
                CalendarObject::Task(_) => {
                    return Err(Error::Conflict("only events can be invited to".into()));
                }
            };
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
        let iri = self.get(id).await?.ok_or(Error::NotFound(id))?.envelope.iri;
        let mut obj = self.get_object(id).await?.ok_or(Error::NotFound(id))?;
        if let CalendarObject::Event(e) = &obj {
            if e.status.as_deref() == Some("cancelled") {
                return Err(Error::Conflict("the event is cancelled".into()));
            }
        }
        let key = participant_key(&obj, who)?;
        let p = obj
            .participants_mut()
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
fn participant_key(obj: &CalendarObject, who: &str) -> Result<String> {
    if obj.participants().contains_key(who) {
        return Ok(who.to_string());
    }
    match obj {
        CalendarObject::Event(e) => e.participant_by_email(who).map(str::to_string),
        CalendarObject::Task(t) => t.participant_by_email(who).map(str::to_string),
    }
    .ok_or_else(|| Error::Conflict(format!("{who} is not a participant")))
}
