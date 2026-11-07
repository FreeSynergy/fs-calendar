//! `list` (every page) and `occurrences`.
//!
//! Pattern: Iterator, external — `list` takes a page, then the cursor it was given, until the cursor
//! is empty. The calendar never shows this list by pages (its views are windows of time), so the
//! pages are walked inside the call and the caller gets the whole list.

use chrono::{DateTime, Utc};
use fs_calendar_core::{Instance, Result};
use fs_interfaces::events_capnp::items::Client;
use fs_interfaces::rpc::to_millis;
use fs_store::StoredObject;

use crate::problems::{Subject, error_of};
use crate::read::{occurrence_of, stored, unreadable};

/// Every event, page by page.
pub(crate) async fn list(c: &Client) -> Result<Vec<StoredObject>> {
    let mut found: Vec<StoredObject> = Vec::new();
    let mut cursor: Vec<u8> = Vec::new();
    loop {
        let before = found.len();
        let mut req = c.list_request();
        req.get().init_cursor().set_opaque(&cursor);
        let answer = req
            .send()
            .promise
            .await
            .map_err(|e| error_of(&e, &Subject::None))?;
        let page = answer
            .get()
            .and_then(|r| r.get_page())
            .map_err(unreadable)?;
        for item in page.get_items().map_err(unreadable)? {
            found.push(stored(item)?);
        }
        cursor = page
            .get_next()
            .and_then(|n| n.get_opaque())
            .map_err(unreadable)?
            .to_vec();
        if cursor.is_empty() {
            return Ok(found);
        }
        if found.len() == before {
            return Err(unreadable("an empty page that names a next one"));
        }
    }
}

/// `occurrences`: the instances in `[from, to)`.
pub(crate) async fn occurrences(
    c: &Client,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> Result<Vec<Instance>> {
    let mut req = c.occurrences_request();
    req.get().init_from().set_unix_millis(to_millis(from));
    req.get().init_to().set_unix_millis(to_millis(to));
    let answer = req
        .send()
        .promise
        .await
        .map_err(|e| error_of(&e, &Subject::None))?;
    answer
        .get()
        .and_then(|r| r.get_items())
        .map_err(unreadable)?
        .iter()
        .map(occurrence_of)
        .collect()
}
