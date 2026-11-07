//! `importText` and `exportText`.
//!
//! Pattern: Adapter — a text and a media type out, a report or a text back.

use fs_calendar_core::{ImportReport, Result};
use fs_interfaces::events_capnp::items::Client;
use fs_interfaces::rpc::write_object_id;
use fs_store::ObjectId;

use crate::problems::{Subject, error_of};
use crate::read::{report, unreadable};

/// `importText`.
pub(crate) async fn import_text(
    c: &Client,
    media_type: String,
    text: String,
) -> Result<ImportReport> {
    let mut req = c.import_text_request();
    req.get().set_media_type(media_type.as_str());
    req.get().set_text(text.as_str());
    let answer = req
        .send()
        .promise
        .await
        .map_err(|e| error_of(&e, &Subject::None))?;
    report(
        answer
            .get()
            .and_then(|r| r.get_report())
            .map_err(unreadable)?,
    )
}

/// `exportText`.
pub(crate) async fn export_text(
    c: &Client,
    ids: Vec<ObjectId>,
    media_type: String,
) -> Result<String> {
    let mut req = c.export_text_request();
    req.get().set_media_type(media_type.as_str());
    let mut list = req
        .get()
        .init_ids(u32::try_from(ids.len()).map_err(unreadable)?);
    for (i, id) in (0u32..).zip(&ids) {
        write_object_id(list.reborrow().get(i), *id);
    }
    let answer = req.send().promise.await.map_err(|e| {
        error_of(
            &e,
            &ids.first().map_or(Subject::None, |id| Subject::Event(*id)),
        )
    })?;
    let r = answer.get().map_err(unreadable)?;
    r.get_text()
        .map_err(unreadable)?
        .to_string()
        .map_err(unreadable)
}
