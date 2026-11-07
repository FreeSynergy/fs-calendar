//! `subscribe` over a real socket: every kind of change, and nothing after the drop.

mod common;

use std::cell::RefCell;
use std::rc::Rc;

use capnp_rpc::new_client;
use common::{create, invitations_of, items_of, set_id, with_server};
use fs_interfaces::common_capnp::{change, change_listener};
use fs_interfaces::rpc::read_envelope;

#[derive(Clone, Default)]
struct Seen(Rc<RefCell<Vec<String>>>);

impl change_listener::Server for Seen {
    async fn change(
        self: Rc<Self>,
        params: change_listener::ChangeParams,
    ) -> Result<(), capnp::Error> {
        let kind = match params.get()?.get_change()?.which()? {
            change::Which::Created(e) => format!("created {}", read_envelope(e?)?.name),
            change::Which::Updated(e) => format!("updated {}", read_envelope(e?)?.name),
            change::Which::Deleted(_) => "deleted".to_string(),
        };
        self.0.borrow_mut().push(kind);
        Ok(())
    }
}

async fn settle() {
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
}

#[tokio::test(flavor = "current_thread")]
async fn subscribers_see_changes_until_they_drop_the_capability() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        let invitations = invitations_of(&root).await;
        let seen = Seen::default();
        let listener: change_listener::Client = new_client(seen.clone());
        let mut sub = root.subscribe_request();
        sub.get().set_listener(listener);
        let answer = sub.send().promise.await.unwrap();
        let subscription = answer.get().unwrap().get_subscription().unwrap();

        let id = create(&items, "uid-1", "Watched", "2026-12-01T10:00:00").await;
        let mut invite = invitations.invite_request();
        set_id(invite.get().init_id(), id);
        let mut one = invite.get().init_invitees(1).get(0);
        one.set_name("Ben");
        one.set_email("ben@example.org");
        invite.send().promise.await.unwrap();
        let mut delete = items.delete_request();
        set_id(delete.get().init_id(), id);
        delete.send().promise.await.unwrap();
        settle().await;
        assert_eq!(
            *seen.0.borrow(),
            ["created Watched", "updated Watched", "deleted"]
        );

        drop(subscription);
        drop(answer);
        settle().await;
        create(&items, "uid-2", "Unwatched", "2026-12-01T10:00:00").await;
        settle().await;
        assert_eq!(seen.0.borrow().len(), 3, "still told after the drop");
    })
    .await;
}
