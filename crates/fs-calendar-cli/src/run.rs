//! Carrying out one command against an `events::Client`.
//!
//! Pattern: Command (executor) — one function per command, each a few contract calls. Errors come
//! back as the text of the problem the server sent; the contract's problem types are not reinterpreted.

use fs_interfaces::common_capnp;
use fs_interfaces::events_capnp::{event, events, items};
use fs_interfaces::rpc::{read_envelope, write_uuid};
use fs_objects::ObjectId;
use serde_json::json;

use crate::args::Command;
use crate::show::{line, pretty};

type Out = Result<String, String>;

fn failed(e: &capnp::Error) -> String {
    e.to_string()
}

fn id_of(text: &str) -> Result<ObjectId, String> {
    text.trim()
        .parse()
        .map_err(|_| format!("{text:?} is not an event id"))
}

/// Run `command`; the text to print.
///
/// # Errors
/// The server refused (its problem text), or an id or a file could not be read.
pub async fn run(root: &events::Client, command: Command) -> Out {
    let items = root
        .items_request()
        .send()
        .promise
        .await
        .map_err(|e| failed(&e))?
        .get()
        .and_then(events::items_results::Reader::get_items)
        .map_err(|e| failed(&e))?;
    match command {
        Command::List => list(&items).await,
        Command::Show(id) => show(&items, id_of(&id)?).await,
        Command::Add {
            title,
            start,
            zone,
            duration,
        } => add(&items, &title, &start, zone, duration).await,
        Command::Delete(id) => delete(&items, id_of(&id)?).await,
        Command::Export => export(&items).await,
        Command::Import(path) => import(&items, &path).await,
    }
}

fn one(e: event::Reader<'_>) -> Result<(ObjectId, String), String> {
    let id = read_envelope(e.get_envelope().map_err(|e| failed(&e))?)
        .map_err(|e| failed(&e))?
        .id;
    let text = e
        .get_jscalendar()
        .map_err(|e| failed(&e))?
        .to_string()
        .map_err(|e| e.to_string())?;
    Ok((id, text))
}

/// Every event, page by page.
async fn all(items: &items::Client) -> Result<Vec<(ObjectId, String)>, String> {
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
            found.push(one(e)?);
        }
        let next = page
            .get_next()
            .and_then(common_capnp::cursor::Reader::get_opaque)
            .map_err(|e| failed(&e))?;
        if next.is_empty() {
            return Ok(found);
        }
        cursor = next.to_vec();
    }
}

async fn list(items: &items::Client) -> Out {
    let lines: Vec<String> = all(items)
        .await?
        .iter()
        .map(|(id, text)| line(&id.to_string(), text))
        .collect();
    Ok(lines.join("\n"))
}

async fn show(items: &items::Client, id: ObjectId) -> Out {
    let mut req = items.get_request();
    write_uuid(req.get().init_id(), Some(id.as_uuid()));
    let answer = req.send().promise.await.map_err(|e| failed(&e))?;
    let r = answer.get().map_err(|e| failed(&e))?;
    if !r.get_found() {
        return Err(format!("no event {id}"));
    }
    Ok(pretty(&one(r.get_event().map_err(|e| failed(&e))?)?.1))
}

async fn add(
    items: &items::Client,
    title: &str,
    start: &str,
    zone: Option<String>,
    duration: Option<String>,
) -> Out {
    let event = json!({
        "@type": "Event", "version": "2.0",
        "uid": ObjectId::new().to_string(), "title": title, "start": start,
        "timeZone": zone.unwrap_or_else(|| "Etc/UTC".into()),
        "duration": duration.unwrap_or_else(|| "PT1H".into()),
    });
    let mut req = items.create_request();
    req.get().set_jscalendar(event.to_string().as_str());
    let answer = req.send().promise.await.map_err(|e| failed(&e))?;
    let (id, text) = one(answer
        .get()
        .and_then(items::create_results::Reader::get_event)
        .map_err(|e| failed(&e))?)?;
    Ok(line(&id.to_string(), &text))
}

async fn delete(items: &items::Client, id: ObjectId) -> Out {
    let mut req = items.delete_request();
    write_uuid(req.get().init_id(), Some(id.as_uuid()));
    req.send().promise.await.map_err(|e| failed(&e))?;
    Ok(format!("deleted {id}"))
}

async fn export(items: &items::Client) -> Out {
    let ids: Vec<ObjectId> = all(items).await?.into_iter().map(|e| e.0).collect();
    let mut req = items.export_text_request();
    req.get().set_media_type("text/calendar");
    let mut list = req
        .get()
        .init_ids(u32::try_from(ids.len()).map_err(|_| "too many events".to_string())?);
    for (i, id) in (0u32..).zip(&ids) {
        write_uuid(list.reborrow().get(i), Some(id.as_uuid()));
    }
    let answer = req.send().promise.await.map_err(|e| failed(&e))?;
    answer
        .get()
        .and_then(items::export_text_results::Reader::get_text)
        .map_err(|e| failed(&e))?
        .to_string()
        .map_err(|e| e.to_string())
}

async fn import(items: &items::Client, path: &std::path::Path) -> Out {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut req = items.import_text_request();
    req.get().set_media_type("text/calendar");
    req.get().set_text(text.as_str());
    let answer = req.send().promise.await.map_err(|e| failed(&e))?;
    let report = answer
        .get()
        .and_then(items::import_text_results::Reader::get_report)
        .map_err(|e| failed(&e))?;
    let created = report.get_created().map_err(|e| failed(&e))?.len();
    let skipped = report.get_skipped().map_err(|e| failed(&e))?.len();
    let failed_n = report.get_failed().map_err(|e| failed(&e))?.len();
    Ok(format!(
        "created {created}, skipped {skipped}, failed {failed_n}"
    ))
}
