//! The calls of `Items` the calendar makes.
//!
//! Pattern: Adapter — one request in, values of the calendar's own types out. `query` walks the pages
//! until the cursor is empty; a task is its envelope and its JSCalendar text, which is not read here.

use chrono::{DateTime, Utc};
use fs_calendar_core::{DueTask, ImportReport, TasksError};
use fs_interfaces::rpc::{read_envelope, read_object_id};
use fs_interfaces::tasks_capnp::{import_report, items, task};

use crate::problems::{Said, error_of, said, unreadable};

/// A cursor that is asked again with the same bytes would loop for ever; the server never does it.
const MAX_PAGES: usize = 100_000;

fn due_task(t: task::Reader<'_>) -> Result<DueTask, TasksError> {
    let envelope = read_envelope(t.get_envelope().map_err(unreadable)?).map_err(unreadable)?;
    let text = t
        .get_jscalendar()
        .map_err(unreadable)?
        .to_str()
        .map_err(unreadable)?;
    Ok(DueTask {
        id: envelope.id,
        envelope,
        jscalendar: serde_json::from_str(text).map_err(unreadable)?,
    })
}

/// The tasks due in `[from, to)`, every page.
pub(crate) async fn due_between(
    items: &items::Client,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> Result<Vec<DueTask>, TasksError> {
    let mut found = Vec::new();
    let mut cursor: Vec<u8> = Vec::new();
    for _ in 0..MAX_PAGES {
        let mut request = items.query_request();
        {
            let mut p = request.get();
            let mut filter = p.reborrow().init_filter();
            filter
                .reborrow()
                .init_due_from()
                .set_unix_millis(from.timestamp_millis());
            filter.init_due_to().set_unix_millis(to.timestamp_millis());
            p.init_cursor().set_opaque(&cursor);
        }
        let answer = request.send().promise.await.map_err(|e| error_of(&e))?;
        let page = answer
            .get()
            .and_then(|r| r.get_page())
            .map_err(unreadable)?;
        for t in page.get_items().map_err(unreadable)? {
            found.push(due_task(t)?);
        }
        let next = page
            .get_next()
            .and_then(|n| n.get_opaque())
            .map_err(unreadable)?;
        if next.is_empty() || next == cursor.as_slice() {
            return Ok(found);
        }
        cursor = next.to_vec();
    }
    Err(TasksError::new("tasks server: pages without an end"))
}

/// Does a task with this `uid` exist?
pub(crate) async fn exists(items: &items::Client, uid: &str) -> Result<bool, TasksError> {
    let mut request = items.get_by_uid_request();
    request.get().set_uid(uid);
    let answer = request.send().promise.await.map_err(|e| error_of(&e))?;
    Ok(answer.get().map_err(unreadable)?.get_found())
}

/// What `create` made of a task: its id, the reason the server refuses it, or that it exists.
pub(crate) enum Created {
    Id(fs_objects::ObjectId),
    Refused(String),
    Exists,
}

/// `create` of the JSCalendar text.
pub(crate) async fn create(items: &items::Client, jscalendar: &str) -> Result<Created, TasksError> {
    let mut request = items.create_request();
    request.get().set_jscalendar(jscalendar);
    match request.send().promise.await {
        Ok(answer) => {
            let task = answer
                .get()
                .and_then(|r| r.get_task())
                .map_err(unreadable)?;
            let envelope =
                read_envelope(task.get_envelope().map_err(unreadable)?).map_err(unreadable)?;
            Ok(Created::Id(envelope.id))
        }
        Err(e) => match said(&e) {
            Some(Said::Refused(why)) => Ok(Created::Refused(why)),
            Some(Said::Exists) => Ok(Created::Exists),
            None => Err(error_of(&e)),
        },
    }
}

/// `importIcs`: the server's report as the calendar's.
pub(crate) async fn import_ics(
    items: &items::Client,
    text: &str,
) -> Result<ImportReport, TasksError> {
    let mut request = items.import_ics_request();
    request.get().set_text(text);
    let answer = request.send().promise.await.map_err(|e| error_of(&e))?;
    let report = answer
        .get()
        .and_then(|r| r.get_report())
        .map_err(unreadable)?;
    report_of(report)
}

fn report_of(report: import_report::Reader<'_>) -> Result<ImportReport, TasksError> {
    let texts = |list: capnp::text_list::Reader<'_>| -> Result<Vec<String>, TasksError> {
        list.iter()
            .map(|t| t.map_err(unreadable)?.to_string().map_err(unreadable))
            .collect()
    };
    Ok(ImportReport {
        created: report
            .get_created()
            .map_err(unreadable)?
            .iter()
            .map(|id| read_object_id(id).map_err(unreadable))
            .collect::<Result<_, _>>()?,
        skipped: texts(report.get_skipped().map_err(unreadable)?)?,
        failed: texts(report.get_failed().map_err(unreadable)?)?,
        notes: texts(report.get_notes().map_err(unreadable)?)?,
    })
}
