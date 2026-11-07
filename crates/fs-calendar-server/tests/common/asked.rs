//! Helpers that read answers: every page of a list, and the answer of a window.

#![allow(dead_code)] // each test file uses a different subset

use fs_interfaces::events_capnp::items;
use fs_interfaces::rpc::read_envelope;
use uuid::Uuid;

use super::{event_json, ms};

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
