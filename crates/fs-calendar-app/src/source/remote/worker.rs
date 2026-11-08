//! The worker thread: owns the connection, carries out the calls.
//!
//! Pattern: Command — each [`Call`] is carried out against the server's `Items`, one task each.

use std::path::Path;

use chrono::{DateTime, Utc};
use fs_calendar_core::{Result, Window};
use fs_interfaces::events_capnp::{events, items};
use fs_interfaces::rpc::{connect, to_millis, write_object_id};
use fs_objects::{ObjectId, StoredObject};
use tokio::sync::{mpsc, oneshot};

use super::convert::{damaged_of, failed, instance_of, stored};

pub(super) type Reply<T> = oneshot::Sender<Result<T>>;

pub(super) enum Call {
    Get(ObjectId, Reply<Option<StoredObject>>),
    ByUid(String, Reply<Option<StoredObject>>),
    List(Reply<Vec<StoredObject>>),
    Window(DateTime<Utc>, DateTime<Utc>, Reply<Window>),
    Create(String, Reply<StoredObject>),
    Update(ObjectId, String, Reply<StoredObject>),
    Delete(ObjectId, Reply<()>),
}

type Ready = std::sync::mpsc::Sender<std::result::Result<(), String>>;

pub(super) fn worker(socket: &Path, mut inbox: mpsc::UnboundedReceiver<Call>, ready: &Ready) {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(r) => r,
        Err(e) => {
            let _ = ready.send(Err(e.to_string()));
            return;
        }
    };
    let local = tokio::task::LocalSet::new();
    local.block_on(&runtime, async move {
        let connection = match connect::<events::Client>(socket).await {
            Ok(c) => c,
            Err(e) => {
                let _ = ready.send(Err(format!(
                    "no calendar server on {}: {e}; start it with `fs-calendar serve`",
                    socket.display()
                )));
                return;
            }
        };
        let items = match open_items(&connection.client).await {
            Ok(items) => items,
            Err(e) => {
                let _ = ready.send(Err(e.to_string()));
                return;
            }
        };
        let _ = ready.send(Ok(()));
        while let Some(call) = inbox.recv().await {
            let items = items.clone();
            tokio::task::spawn_local(async move { carry_out(&items, call).await });
        }
    });
}

async fn open_items(root: &events::Client) -> capnp::Result<items::Client> {
    root.items_request()
        .send()
        .promise
        .await?
        .get()?
        .get_items()
}

async fn carry_out(items: &items::Client, call: Call) {
    match call {
        Call::Get(id, reply) => {
            let _ = reply.send(get(items, id).await);
        }
        Call::ByUid(uid, reply) => {
            let _ = reply.send(by_uid(items, &uid).await);
        }
        Call::List(reply) => {
            let _ = reply.send(list(items).await);
        }
        Call::Window(from, to, reply) => {
            let _ = reply.send(window(items, from, to).await);
        }
        Call::Create(text, reply) => {
            let _ = reply.send(create(items, &text).await);
        }
        Call::Update(id, text, reply) => {
            let _ = reply.send(update(items, id, &text).await);
        }
        Call::Delete(id, reply) => {
            let _ = reply.send(delete(items, id).await);
        }
    }
}

async fn get(items: &items::Client, id: ObjectId) -> Result<Option<StoredObject>> {
    let mut req = items.get_request();
    write_object_id(req.get().init_id(), id);
    let answer = req.send().promise.await.map_err(|e| failed(&e))?;
    let r = answer.get().map_err(|e| failed(&e))?;
    if !r.get_found() {
        return Ok(None);
    }
    stored(r.get_event().map_err(|e| failed(&e))?).map(Some)
}

async fn by_uid(items: &items::Client, uid: &str) -> Result<Option<StoredObject>> {
    let mut req = items.get_by_uid_request();
    req.get().set_uid(uid);
    let answer = req.send().promise.await.map_err(|e| failed(&e))?;
    let r = answer.get().map_err(|e| failed(&e))?;
    if !r.get_found() {
        return Ok(None);
    }
    stored(r.get_event().map_err(|e| failed(&e))?).map(Some)
}

async fn list(items: &items::Client) -> Result<Vec<StoredObject>> {
    let mut found = vec![];
    let mut cursor: Vec<u8> = vec![];
    loop {
        let mut req = items.list_request();
        req.get().init_cursor().set_opaque(&cursor);
        let answer = req.send().promise.await.map_err(|e| failed(&e))?;
        let page = answer
            .get()
            .and_then(items::list_results::Reader::get_page)
            .map_err(|e| failed(&e))?;
        for e in page.get_items().map_err(|e| failed(&e))? {
            found.push(stored(e)?);
        }
        let next = page
            .get_next()
            .and_then(fs_interfaces::common_capnp::cursor::Reader::get_opaque)
            .map_err(|e| failed(&e))?;
        if next.is_empty() || next == cursor.as_slice() {
            return Ok(found);
        }
        cursor = next.to_vec();
    }
}

async fn window(items: &items::Client, from: DateTime<Utc>, to: DateTime<Utc>) -> Result<Window> {
    let mut req = items.occurrences_request();
    req.get().init_from().set_unix_millis(to_millis(from));
    req.get().init_to().set_unix_millis(to_millis(to));
    let answer = req.send().promise.await.map_err(|e| failed(&e))?;
    let r = answer.get().map_err(|e| failed(&e))?;
    let mut instances = vec![];
    for i in r.get_items().map_err(|e| failed(&e))? {
        instances.push(instance_of(i)?);
    }
    let mut damaged = vec![];
    for d in r.get_damaged().map_err(|e| failed(&e))? {
        damaged.push(damaged_of(d)?);
    }
    Ok(Window { instances, damaged })
}

async fn create(items: &items::Client, text: &str) -> Result<StoredObject> {
    let mut req = items.create_request();
    req.get().set_jscalendar(text);
    let answer = req.send().promise.await.map_err(|e| failed(&e))?;
    stored(
        answer
            .get()
            .and_then(items::create_results::Reader::get_event)
            .map_err(|e| failed(&e))?,
    )
}

async fn update(items: &items::Client, id: ObjectId, text: &str) -> Result<StoredObject> {
    let mut req = items.update_request();
    write_object_id(req.get().init_id(), id);
    req.get().set_jscalendar(text);
    let answer = req.send().promise.await.map_err(|e| failed(&e))?;
    stored(
        answer
            .get()
            .and_then(items::update_results::Reader::get_event)
            .map_err(|e| failed(&e))?,
    )
}

async fn delete(items: &items::Client, id: ObjectId) -> Result<()> {
    let mut req = items.delete_request();
    write_object_id(req.get().init_id(), id);
    req.send().promise.await.map_err(|e| failed(&e))?;
    Ok(())
}
