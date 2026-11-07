//! Shared helpers of the integration tests: a real server on a real Unix socket.

#![allow(dead_code)] // each test file uses a different subset

use std::sync::Arc;

use fs_calendar_core::{Calendar, CalendarFeed, CalendarRepo};
use fs_calendar_server::CalendarServer;
use fs_interfaces::common_capnp;
use fs_interfaces::events_capnp::{self, events, invitations, items};
use fs_interfaces::rpc::{bind_socket, connect, read_envelope, serve, write_uuid};
use fs_store::{Store, TursoStore};
use uuid::Uuid;

pub const ME: &str = "https://local.invalid/users/owner";

/// An event as JSCalendar text; Berlin, one hour.
pub fn json(uid: &str, title: &str, start: &str) -> String {
    format!(
        r#"{{"@type":"Event","version":"2.0","uid":"{uid}","title":"{title}","start":"{start}","timeZone":"Europe/Berlin","duration":"PT1H"}}"#
    )
}

pub async fn with_server<F, Fut>(test: F)
where
    F: FnOnce(events::Client) -> Fut,
    Fut: Future<Output = ()>,
{
    with_store(|root, _| test(root)).await;
}

/// A server over a repository, and the store behind it for what only old data can show.
pub async fn with_store<F, Fut>(test: F)
where
    F: FnOnce(events::Client, Arc<dyn Store>) -> Fut,
    Fut: Future<Output = ()>,
{
    let store: Arc<dyn Store> = Arc::new(TursoStore::open_in_memory().await.unwrap());
    let repo = Arc::new(CalendarRepo::new(store.clone(), ME));
    with_provider(repo.clone(), repo, |root| test(root, store)).await;
}

/// A real server in front of this provider.
pub async fn with_provider<F, Fut>(
    calendar: Arc<dyn Calendar>,
    feed: Arc<dyn CalendarFeed>,
    test: F,
) where
    F: FnOnce(events::Client) -> Fut,
    Fut: Future<Output = ()>,
{
    with_calendar_server(CalendarServer::new(calendar, feed), test).await;
}

/// A real server on a real socket, as built by the caller.
pub async fn with_calendar_server<F, Fut>(server: CalendarServer, test: F)
where
    F: FnOnce(events::Client) -> Fut,
    Fut: Future<Output = ()>,
{
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("events.sock");
            let root = server.into_client();
            let bound = bind_socket(&path).await.unwrap();
            let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
            let serving = tokio::task::spawn_local(serve(bound, root.client, async {
                let _ = stopped.await;
            }));
            let connection = connect::<events::Client>(&path).await.unwrap();
            test(connection.client.clone()).await;
            drop(connection);
            let _ = stop.send(());
            serving.await.unwrap().unwrap();
        })
        .await;
}

pub async fn items_of(root: &events::Client) -> items::Client {
    root.items_request()
        .send()
        .promise
        .await
        .unwrap()
        .get()
        .unwrap()
        .get_items()
        .unwrap()
}

pub async fn invitations_of(root: &events::Client) -> invitations::Client {
    root.invitations_request()
        .send()
        .promise
        .await
        .unwrap()
        .get()
        .unwrap()
        .get_invitations()
        .unwrap()
}

/// Create an event and answer its object id.
pub async fn create(items: &items::Client, uid: &str, title: &str, start: &str) -> Uuid {
    let mut req = items.create_request();
    req.get().set_jscalendar(json(uid, title, start).as_str());
    let answer = req.send().promise.await.unwrap();
    let event = answer.get().unwrap().get_event().unwrap();
    let env = read_envelope(event.get_envelope().unwrap()).unwrap();
    assert_eq!(env.name, title);
    assert_eq!(env.context.as_deref(), Some("calendar"));
    assert_eq!(env.object_type, "Event");
    env.id.as_uuid()
}

/// Write an event id.
pub fn set_id(out: common_capnp::uuid::Builder<'_>, id: Uuid) {
    write_uuid(out, Some(id));
}

/// The JSCalendar content of an event on the wire.
pub fn event_json(event: events_capnp::event::Reader<'_>) -> serde_json::Value {
    serde_json::from_str(event.get_jscalendar().unwrap().to_str().unwrap()).unwrap()
}

/// Milliseconds of a UTC time given as text.
pub fn ms(utc: &str) -> i64 {
    fs_calendar_core::utc(utc).unwrap().timestamp_millis()
}

/// A server over a fresh store that gives pages of at most `size` events.
pub async fn with_page_size<F, Fut>(size: usize, test: F)
where
    F: FnOnce(events::Client) -> Fut,
    Fut: Future<Output = ()>,
{
    let store: Arc<dyn Store> = Arc::new(TursoStore::open_in_memory().await.unwrap());
    let repo = Arc::new(CalendarRepo::new(store, ME));
    let server = CalendarServer::new(repo.clone(), repo).with_page_size(size);
    with_calendar_server(server, test).await;
}

/// The titles of every page of `Items.list`, in the order the server gave them; `between` runs once,
/// after the first page, with the object ids of that page.
pub async fn read_pages<F, Fut>(items: &items::Client, between: F) -> Vec<String>
where
    F: FnOnce(Vec<Uuid>) -> Fut,
    Fut: Future<Output = ()>,
{
    let (mut titles, mut cursor, mut between) = (vec![], vec![], Some(between));
    loop {
        let mut call = items.list_request();
        call.get().init_cursor().set_opaque(&cursor);
        let answer = call.send().promise.await.unwrap();
        let page = answer.get().unwrap().get_page().unwrap();
        let mut ids = vec![];
        for event in page.get_items().unwrap() {
            titles.push(event_json(event)["uid"].as_str().unwrap().to_string());
            ids.push(
                read_envelope(event.get_envelope().unwrap())
                    .unwrap()
                    .id
                    .as_uuid(),
            );
        }
        cursor = page.get_next().unwrap().get_opaque().unwrap().to_vec();
        if let Some(between) = between.take() {
            between(ids).await;
        }
        if cursor.is_empty() {
            return titles;
        }
    }
}

/// What `Items.occurrences` answered: the object of each instance, what is damaged, and whether it was cut.
pub struct Asked {
    pub objects: Vec<Uuid>,
    /// `(event id, reason, key)` of each entry of `damaged`.
    pub damaged: Vec<(Uuid, String, String)>,
    pub cut: bool,
}

/// Ask for the instances in `[from, to)` (UTC texts).
pub async fn ask_window(items: &items::Client, from: &str, to: &str) -> capnp::Result<Asked> {
    let mut req = items.occurrences_request();
    req.get().init_from().set_unix_millis(ms(from));
    req.get().init_to().set_unix_millis(ms(to));
    let answer = req.send().promise.await?;
    let answer = answer.get()?;
    let mut objects = vec![];
    for instance in answer.get_items()? {
        objects.push(fs_interfaces::rpc::read_uuid(instance.get_object()?).unwrap());
    }
    let mut damaged = vec![];
    for d in answer.get_damaged()? {
        damaged.push((
            fs_interfaces::rpc::read_uuid(d.get_id()?).unwrap(),
            d.get_reason()?.to_string()?,
            d.get_key()?.to_string()?,
        ));
    }
    Ok(Asked {
        objects,
        damaged,
        cut: answer.get_cut(),
    })
}

/// Create a JSCalendar event from its JSON and answer its object id.
pub async fn create_value(items: &items::Client, event: &serde_json::Value) -> Uuid {
    let mut call = items.create_request();
    call.get().set_jscalendar(event.to_string().as_str());
    let answer = call.send().promise.await.unwrap();
    let event = answer.get().unwrap().get_event().unwrap();
    read_envelope(event.get_envelope().unwrap())
        .unwrap()
        .id
        .as_uuid()
}
