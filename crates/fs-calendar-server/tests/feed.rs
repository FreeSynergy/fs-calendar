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

#[allow(clippy::unused_async_trait_impl)] // the generated trait requires async methods
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

struct Ends(Rc<RefCell<Vec<String>>>);

#[allow(clippy::unused_async_trait_impl)] // the generated trait requires async methods
impl change_listener::Server for Ends {
    async fn change(self: Rc<Self>, _: change_listener::ChangeParams) -> Result<(), capnp::Error> {
        Ok(())
    }

    async fn ended(
        self: Rc<Self>,
        params: change_listener::EndedParams,
        _: change_listener::EndedResults,
    ) -> Result<(), capnp::Error> {
        let problem = params.get()?.get_problem()?;
        self.0.borrow_mut().push(format!(
            "{} {}",
            problem.get_type()?.to_str()?,
            problem.get_detail()?.to_str()?
        ));
        Ok(())
    }
}

/// A provider that cannot read an event back: a change cannot be told, so the feed must end loudly.
struct Unreadable;

#[async_trait::async_trait]
impl fs_calendar_core::Calendar for Unreadable {
    fn capabilities(&self) -> fs_calendar_core::Capabilities {
        fs_calendar_core::Capabilities::new("unreadable")
    }
    async fn get(
        &self,
        _: fs_objects::ObjectId,
    ) -> fs_calendar_core::Result<Option<fs_objects::StoredObject>> {
        Err(fs_calendar_core::Error::Conflict(
            "the store does not answer".into(),
        ))
    }
    async fn get_event(
        &self,
        _: fs_objects::ObjectId,
    ) -> fs_calendar_core::Result<Option<fs_calendar_core::Event>> {
        Ok(None)
    }
    async fn get_by_uid(
        &self,
        _: &str,
    ) -> fs_calendar_core::Result<Option<fs_objects::StoredObject>> {
        Ok(None)
    }
    async fn list_events(&self) -> fs_calendar_core::Result<Vec<fs_objects::StoredObject>> {
        Ok(vec![])
    }
    async fn occurrences(
        &self,
        _: chrono::DateTime<chrono::Utc>,
        _: chrono::DateTime<chrono::Utc>,
    ) -> fs_calendar_core::Result<Vec<fs_calendar_core::Instance>> {
        Ok(vec![])
    }
}

#[tokio::test(flavor = "current_thread")]
async fn a_feed_that_cannot_tell_a_change_ends_and_says_so() {
    let store: std::sync::Arc<dyn fs_objects::Store> =
        std::sync::Arc::new(fs_objects::TursoStore::open_in_memory().await.unwrap());
    let repo = std::sync::Arc::new(fs_calendar_core::CalendarRepo::new(store, common::ME));
    common::with_provider(
        std::sync::Arc::new(Unreadable),
        repo.clone(),
        |root| async move {
            let ended = Rc::new(RefCell::new(Vec::new()));
            let listener: change_listener::Client = new_client(Ends(ended.clone()));
            let mut sub = root.subscribe_request();
            sub.get().set_listener(listener);
            let answer = sub.send().promise.await.unwrap();
            let mut event =
                fs_calendar_core::Event::new("E", "2026-12-01T10:00:00", "Etc/UTC", "PT1H");
            event.uid = "e".into();
            repo.create(event).await.unwrap();
            settle().await;
            let seen = ended.borrow().clone();
            assert_eq!(seen.len(), 1, "{seen:?}");
            assert!(
                seen[0].starts_with(fs_interfaces::rpc::FEED_ENDED_PROBLEM_TYPE),
                "{seen:?}"
            );
            assert!(seen[0].contains("does not answer"), "{seen:?}");
            drop(answer);
        },
    )
    .await;
}
