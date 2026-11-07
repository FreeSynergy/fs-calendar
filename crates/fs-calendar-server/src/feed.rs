//! Changes of the provider to a subscribed consumer.
//!
//! Pattern: Observer + Adapter — the core calls its listener on the thread that made the change; this
//! one only puts the change into a channel. The shared forwarder adds the envelope and sends it.

use std::sync::Arc;

use fs_calendar_core::{Calendar, CalendarChange, CalendarFeed, CalendarListener};
use fs_interfaces::common_capnp::{change_listener, subscription};
use fs_interfaces::rpc::{ChangeSender, Notice, change_channel, forward};

struct ToChannel(ChangeSender<CalendarChange>);

impl CalendarListener for ToChannel {
    fn changed(&self, change: &CalendarChange) {
        self.0.send(*change);
    }
}

/// Subscribe `listener` until the returned capability is dropped.
pub(crate) fn subscribe(
    feed: &Arc<dyn CalendarFeed>,
    calendar: Arc<dyn Calendar>,
    listener: change_listener::Client,
) -> subscription::Client {
    let (tx, rx) = change_channel();
    let core_subscription = feed.subscribe(Arc::new(ToChannel(tx)));
    forward(rx, core_subscription, listener, move |change| {
        let calendar = calendar.clone();
        async move { notice(calendar.as_ref(), change).await }
    })
}

/// An event that is gone again by now has nothing to report but its own `Deleted`, which follows. A
/// failed read is something else: the consumer would miss a change, so it ends the feed.
async fn notice(calendar: &dyn Calendar, change: CalendarChange) -> Result<Option<Notice>, String> {
    let envelope = async |id| {
        let object = calendar.get(id).await.map_err(|e| e.to_string())?;
        Ok::<_, String>(object.map(|o| o.envelope))
    };
    Ok(match change {
        CalendarChange::Created(id) => envelope(id).await?.map(Notice::Created),
        CalendarChange::Updated(id) => envelope(id).await?.map(Notice::Updated),
        CalendarChange::Deleted(id) => Some(Notice::Deleted(id)),
    })
}
