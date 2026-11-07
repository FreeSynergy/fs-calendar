//! `create`, `update`, `updateIf`, `delete`, `get`, `getByUid`.
//!
//! Pattern: Adapter — one request in, one reply read back as values of the core.

use fs_calendar_core::{Result, Revision};
use fs_interfaces::events_capnp::items::Client;
use fs_interfaces::rpc::write_object_id;
use fs_store::{ObjectId, StoredObject};

use crate::problems::{Subject, error_of};
use crate::read::{revised, stored, unreadable};
use crate::revised::Revised;

/// `create`.
pub(crate) async fn create(c: &Client, event: String) -> Result<StoredObject> {
    let mut req = c.create_request();
    req.get().set_jscalendar(event.as_str());
    let answer = req
        .send()
        .promise
        .await
        .map_err(|e| error_of(&e, &Subject::None))?;
    stored(
        answer
            .get()
            .and_then(|r| r.get_event())
            .map_err(unreadable)?,
    )
}

/// `update`.
pub(crate) async fn update(c: &Client, id: ObjectId, event: String) -> Result<StoredObject> {
    let mut req = c.update_request();
    write_object_id(req.get().init_id(), id);
    req.get().set_jscalendar(event.as_str());
    let answer = req
        .send()
        .promise
        .await
        .map_err(|e| error_of(&e, &Subject::Event(id)))?;
    stored(
        answer
            .get()
            .and_then(|r| r.get_event())
            .map_err(unreadable)?,
    )
}

/// `updateIf`: only if the event still has `revision`; the server answers `conflict:` otherwise.
pub(crate) async fn update_if(
    c: &Client,
    id: ObjectId,
    revision: Revision,
    event: String,
) -> Result<StoredObject> {
    let mut req = c.update_if_request();
    write_object_id(req.get().init_id(), id);
    req.get().set_revision(revision.as_u64());
    req.get().set_jscalendar(event.as_str());
    let answer = req
        .send()
        .promise
        .await
        .map_err(|e| error_of(&e, &Subject::Event(id)))?;
    stored(
        answer
            .get()
            .and_then(|r| r.get_event())
            .map_err(unreadable)?,
    )
}

/// `delete`.
pub(crate) async fn delete(c: &Client, id: ObjectId) -> Result<()> {
    let mut req = c.delete_request();
    write_object_id(req.get().init_id(), id);
    req.send()
        .promise
        .await
        .map_err(|e| error_of(&e, &Subject::Event(id)))?;
    Ok(())
}

/// `get` with the revision: `None` when the server says `found = false`.
pub(crate) async fn get(c: &Client, id: ObjectId) -> Result<Option<Revised>> {
    let mut req = c.get_request();
    write_object_id(req.get().init_id(), id);
    let answer = req
        .send()
        .promise
        .await
        .map_err(|e| error_of(&e, &Subject::Event(id)))?;
    let r = answer.get().map_err(unreadable)?;
    if !r.get_found() {
        return Ok(None);
    }
    revised(r.get_event().map_err(unreadable)?).map(Some)
}

/// `getByUid`.
pub(crate) async fn get_by_uid(c: &Client, uid: String) -> Result<Option<StoredObject>> {
    let mut req = c.get_by_uid_request();
    req.get().set_uid(uid.as_str());
    let answer = req
        .send()
        .promise
        .await
        .map_err(|e| error_of(&e, &Subject::None))?;
    let r = answer.get().map_err(unreadable)?;
    if !r.get_found() {
        return Ok(None);
    }
    stored(r.get_event().map_err(unreadable)?).map(Some)
}
