//! `Invitations` interface: invite, respond, history.
//!
//! Pattern: Adapter — wire request in, `Calendar` call, wire response out.

use std::rc::Rc;
use std::sync::Arc;

use fs_calendar_core::Calendar;
use fs_calendar_core::contract::response_from_wire;
use fs_interfaces::events_capnp::invitations;
use fs_interfaces::rpc::{read_object_id, run_to_end};

use crate::errors::rpc_error;
use crate::wire::{optional, required, write_event, write_history};

pub(crate) struct InvitationsImpl {
    pub(crate) calendar: Arc<dyn Calendar>,
}

type Done = Result<(), capnp::Error>;

impl invitations::Server for InvitationsImpl {
    async fn invite(
        self: Rc<Self>,
        params: invitations::InviteParams,
        mut results: invitations::InviteResults,
    ) -> Done {
        let p = params.get()?;
        let id = read_object_id(p.get_id()?)?;
        let mut invitees: Vec<(String, String)> = Vec::new();
        for one in p.get_invitees()? {
            let name = optional(one.get_name()?)?.to_string();
            let email = required(one.get_email()?, "email")?.to_string();
            invitees.push((name, email));
        }
        let calendar = self.calendar.clone();
        let object = run_to_end(async move {
            let pairs: Vec<(&str, &str)> = invitees
                .iter()
                .map(|(n, e)| (n.as_str(), e.as_str()))
                .collect();
            calendar.invite(id, &pairs).await
        })
        .await?
        .map_err(rpc_error)?;
        write_event(results.get().init_event(), &object);
        Ok(())
    }

    async fn respond(
        self: Rc<Self>,
        params: invitations::RespondParams,
        mut results: invitations::RespondResults,
    ) -> Done {
        let p = params.get()?;
        let id = read_object_id(p.get_id()?)?;
        let who = required(p.get_who()?, "who")?.to_string();
        let response = response_from_wire(p.get_response()?);
        let comment = optional(p.get_comment()?)?.to_string();
        let calendar = self.calendar.clone();
        let object = run_to_end(async move {
            let comment = (!comment.is_empty()).then_some(comment.as_str());
            calendar.respond(id, &who, response, comment).await
        })
        .await?
        .map_err(rpc_error)?;
        write_event(results.get().init_event(), &object);
        Ok(())
    }

    async fn history(
        self: Rc<Self>,
        params: invitations::HistoryParams,
        results: invitations::HistoryResults,
    ) -> Done {
        let id = read_object_id(params.get()?.get_id()?)?;
        let entries = self.calendar.invitation_log(id).await.map_err(rpc_error)?;
        write_history(results, &entries)
    }
}
