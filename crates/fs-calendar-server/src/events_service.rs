//! `Events` root interface: support report, items, invitations, subscribe.
//!
//! Pattern: Adapter + Factory — the root hands out the `Items` and `Invitations` capabilities.
//! [`CalendarServer`] is what a program serves; it depends on the two traits, never on `CalendarRepo`
//! (ADR 0043).

use std::rc::Rc;
use std::sync::Arc;

use capnp_rpc::new_client;
use fs_calendar_core::contract::CONTRACT_VERSION;
use fs_calendar_core::{Calendar, CalendarFeed};
use fs_interfaces::events_capnp::events;

use crate::feed::subscribe;
use crate::invitations_service::InvitationsImpl;
use crate::items_service::ItemsImpl;

/// The root capability of the calendar program.
pub struct CalendarServer {
    calendar: Arc<dyn Calendar>,
    feed: Arc<dyn CalendarFeed>,
}

impl CalendarServer {
    /// Serve this provider.
    pub fn new(calendar: Arc<dyn Calendar>, feed: Arc<dyn CalendarFeed>) -> Self {
        Self { calendar, feed }
    }

    /// The capability to hand to `fs_interfaces::rpc::serve`.
    #[must_use]
    pub fn into_client(self) -> events::Client {
        new_client(self)
    }
}

type Done = Result<(), capnp::Error>;

impl events::Server for CalendarServer {
    async fn support_report(
        self: Rc<Self>,
        _: events::SupportReportParams,
        mut results: events::SupportReportResults,
    ) -> Done {
        let report = self.calendar.capabilities();
        report.write(CONTRACT_VERSION, results.get().init_report());
        Ok(())
    }

    async fn items(
        self: Rc<Self>,
        _: events::ItemsParams,
        mut results: events::ItemsResults,
    ) -> Done {
        let items = ItemsImpl {
            calendar: self.calendar.clone(),
        };
        results.get().set_items(new_client(items));
        Ok(())
    }

    async fn invitations(
        self: Rc<Self>,
        _: events::InvitationsParams,
        mut results: events::InvitationsResults,
    ) -> Done {
        let invitations = InvitationsImpl {
            calendar: self.calendar.clone(),
        };
        results.get().set_invitations(new_client(invitations));
        Ok(())
    }

    async fn subscribe(
        self: Rc<Self>,
        params: events::SubscribeParams,
        mut results: events::SubscribeResults,
    ) -> Done {
        let listener = params.get()?.get_listener()?;
        let subscription = subscribe(&self.feed, self.calendar.clone(), listener);
        results.get().set_subscription(subscription);
        Ok(())
    }
}
