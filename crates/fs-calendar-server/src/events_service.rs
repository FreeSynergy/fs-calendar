//! `Events` root interface: support report, items, invitations, subscribe.
//!
//! Pattern: Adapter + Factory — the root hands out the `Items` and `Invitations` capabilities.
//! [`CalendarServer`] is what a program serves; it depends on the two traits, never on `CalendarRepo`
//! (ADR 0043).

use std::rc::Rc;
use std::sync::Arc;

use capnp_rpc::new_client;
use fs_calendar_core::contract::CONTRACT_VERSION;
use fs_calendar_core::{Calendar, CalendarFeed, Event};
use fs_interfaces::events_capnp::events;
use fs_interfaces::rpc::{DEFAULT_PAGE_SIZE, write_uuid};
use fs_types::{TypeRegistry, describe::CatalogServer};
use tokio::sync::Semaphore;

use crate::feed::subscribe;
use crate::invitations_service::InvitationsImpl;
use crate::items_service::ItemsImpl;

/// Most requests that write events at once. Each holds the event it has read, up to the limits of the
/// core, while it waits for its turn at the store, so only a few may do that together.
pub const MAX_CONCURRENT_WRITES: usize = 4;

/// The root capability of the calendar program.
pub struct CalendarServer {
    calendar: Arc<dyn Calendar>,
    feed: Arc<dyn CalendarFeed>,
    writers: Arc<Semaphore>,
    page_size: usize,
}

impl CalendarServer {
    /// Serve this provider.
    pub fn new(calendar: Arc<dyn Calendar>, feed: Arc<dyn CalendarFeed>) -> Self {
        Self {
            calendar,
            feed,
            writers: Arc::new(Semaphore::new(MAX_CONCURRENT_WRITES)),
            page_size: DEFAULT_PAGE_SIZE,
        }
    }

    /// At most this many events in a page of `Items.list` (default: the page size of the servers).
    #[must_use]
    pub fn with_page_size(mut self, size: usize) -> Self {
        self.page_size = size.max(1);
        self
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
        let mut out = results.get().init_report();
        report.write(CONTRACT_VERSION, out.reborrow());
        // The store's id, as the store says it: empty where the provider has no store.
        write_uuid(
            out.init_instance(),
            self.calendar.identity().map(|id| id.as_uuid()),
        );
        Ok(())
    }

    async fn items(
        self: Rc<Self>,
        _: events::ItemsParams,
        mut results: events::ItemsResults,
    ) -> Done {
        let items = ItemsImpl {
            calendar: self.calendar.clone(),
            writers: self.writers.clone(),
            page_size: self.page_size,
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

    async fn types(
        self: Rc<Self>,
        _: events::TypesParams,
        mut results: events::TypesResults,
    ) -> Done {
        let mut registry = TypeRegistry::new();
        registry
            .register::<Event>()
            .map_err(|e| capnp::Error::failed(format!("types.unsound: {e:?}")))?;
        results
            .get()
            .set_catalog(new_client(CatalogServer::new(registry)));
        Ok(())
    }
}
