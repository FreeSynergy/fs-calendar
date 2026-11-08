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

use super::convert::{damaged_of, failed, instance_of, stored_with_revision};

pub(super) type Reply<T> = oneshot::Sender<Result<T>>;

/// The server's revision of each event as last read here. The window reads an event right before it
/// writes, so the entry is the revision of what it compared; a rebuilt object could not give it back.
pub(super) type Revisions =
    std::sync::Arc<std::sync::Mutex<std::collections::HashMap<ObjectId, u64>>>;

fn remember(revisions: &Revisions, id: ObjectId, revision: u64) {
    revisions
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(id, revision);
}

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

pub(super) fn worker(
    socket: &Path,
    mut inbox: mpsc::UnboundedReceiver<Call>,
    ready: &Ready,
    revisions: &Revisions,
) {
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
            let (items, revisions) = (items.clone(), std::sync::Arc::clone(revisions));
            tokio::task::spawn_local(async move { carry_out(&items, &revisions, call).await });
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

async fn carry_out(items: &items::Client, revisions: &Revisions, call: Call) {
    match call {
        Call::Get(id, reply) => {
            let _ = reply.send(get(items, revisions, id).await);
        }
        Call::ByUid(uid, reply) => {
            let _ = reply.send(by_uid(items, revisions, &uid).await);
        }
        Call::List(reply) => {
            let _ = reply.send(list(items, revisions).await);
        }
        Call::Window(from, to, reply) => {
            let _ = reply.send(window(items, from, to).await);
        }
        Call::Create(text, reply) => {
            let _ = reply.send(create(items, revisions, &text).await);
        }
        Call::Update(id, text, reply) => {
            let _ = reply.send(update(items, revisions, id, &text).await);
        }
        Call::Delete(id, reply) => {
            let _ = reply.send(delete(items, id).await);
        }
    }
}

async fn get(
    items: &items::Client,
    revisions: &Revisions,
    id: ObjectId,
) -> Result<Option<StoredObject>> {
    let mut req = items.get_request();
    write_object_id(req.get().init_id(), id);
    let answer = req.send().promise.await.map_err(|e| failed(&e))?;
    let r = answer.get().map_err(|e| failed(&e))?;
    if !r.get_found() {
        return Ok(None);
    }
    let (object, revision) = stored_with_revision(r.get_event().map_err(|e| failed(&e))?)?;
    remember(revisions, object.envelope.id, revision);
    Ok(Some(object))
}

async fn by_uid(
    items: &items::Client,
    revisions: &Revisions,
    uid: &str,
) -> Result<Option<StoredObject>> {
    let mut req = items.get_by_uid_request();
    req.get().set_uid(uid);
    let answer = req.send().promise.await.map_err(|e| failed(&e))?;
    let r = answer.get().map_err(|e| failed(&e))?;
    if !r.get_found() {
        return Ok(None);
    }
    let (object, revision) = stored_with_revision(r.get_event().map_err(|e| failed(&e))?)?;
    remember(revisions, object.envelope.id, revision);
    Ok(Some(object))
}

async fn list(items: &items::Client, revisions: &Revisions) -> Result<Vec<StoredObject>> {
    let mut found = vec![];
    let mut cursor: Vec<u8> = vec![];
    // A server that never ends a list cannot grow this without bound.
    for _ in 0..MAX_PAGES {
        let mut req = items.list_request();
        req.get().init_cursor().set_opaque(&cursor);
        let answer = req.send().promise.await.map_err(|e| failed(&e))?;
        let page = answer
            .get()
            .and_then(items::list_results::Reader::get_page)
            .map_err(|e| failed(&e))?;
        for e in page.get_items().map_err(|e| failed(&e))? {
            let (object, revision) = stored_with_revision(e)?;
            remember(revisions, object.envelope.id, revision);
            found.push(object);
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
    Err(fs_calendar_core::Error::Stored(
        "the calendar server's list does not end".into(),
    ))
}

/// The most pages one list reads.
const MAX_PAGES: usize = 10_000;

async fn window(items: &items::Client, from: DateTime<Utc>, to: DateTime<Utc>) -> Result<Window> {
    let mut req = items.occurrences_request();
    req.get().init_from().set_unix_millis(to_millis(from));
    req.get().init_to().set_unix_millis(to_millis(to));
    let answer = req.send().promise.await.map_err(|e| failed(&e))?;
    let r = answer.get().map_err(|e| failed(&e))?;
    let mut instances = vec![];
    let mut damaged = vec![];
    for i in r.get_items().map_err(|e| failed(&e))? {
        // One instance that does not read is reported as damaged; the rest of the window stays.
        match instance_of(i) {
            Ok(found) => instances.push(found),
            Err(e) => {
                if let Ok(id) = i
                    .get_object()
                    .map_err(|e| failed(&e))
                    .and_then(|o| fs_interfaces::rpc::read_object_id(o).map_err(|e| failed(&e)))
                {
                    damaged.push(fs_calendar_core::Damaged {
                        object: id,
                        key: None,
                        reason: e.to_string(),
                        cut: false,
                    });
                }
            }
        }
    }
    for d in r.get_damaged().map_err(|e| failed(&e))? {
        damaged.push(damaged_of(d)?);
    }
    Ok(Window { instances, damaged })
}

async fn create(items: &items::Client, revisions: &Revisions, text: &str) -> Result<StoredObject> {
    let mut req = items.create_request();
    req.get().set_jscalendar(text);
    let answer = req.send().promise.await.map_err(|e| failed(&e))?;
    let (object, revision) = stored_with_revision(
        answer
            .get()
            .and_then(items::create_results::Reader::get_event)
            .map_err(|e| failed(&e))?,
    )?;
    remember(revisions, object.envelope.id, revision);
    Ok(object)
}

/// Written only if the event still has the revision this window last read (`updateIf`): a change made
/// elsewhere meanwhile is a conflict, not silently overwritten. An event never read here has no
/// revision to compare, and is written plainly.
async fn update(
    items: &items::Client,
    revisions: &Revisions,
    id: ObjectId,
    text: &str,
) -> Result<StoredObject> {
    let known = revisions
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(&id)
        .copied();
    let answer_event = if let Some(revision) = known {
        let mut req = items.update_if_request();
        write_object_id(req.get().init_id(), id);
        req.get().set_revision(revision);
        req.get().set_jscalendar(text);
        let answer = req.send().promise.await.map_err(|e| failed(&e))?;
        let (object, revision) = stored_with_revision(
            answer
                .get()
                .and_then(items::update_if_results::Reader::get_event)
                .map_err(|e| failed(&e))?,
        )?;
        (object, revision)
    } else {
        let mut req = items.update_request();
        write_object_id(req.get().init_id(), id);
        req.get().set_jscalendar(text);
        let answer = req.send().promise.await.map_err(|e| failed(&e))?;
        stored_with_revision(
            answer
                .get()
                .and_then(items::update_results::Reader::get_event)
                .map_err(|e| failed(&e))?,
        )?
    };
    remember(revisions, id, answer_event.1);
    Ok(answer_event.0)
}

async fn delete(items: &items::Client, id: ObjectId) -> Result<()> {
    let mut req = items.delete_request();
    write_object_id(req.get().init_id(), id);
    req.send().promise.await.map_err(|e| failed(&e))?;
    Ok(())
}
