//! What a key of the calendar names.
//!
//! Pattern: Newtype with parsing — `<object id>` is a single event, `<object id>#<recurrence id>` one
//! occurrence of a series, `task:<object id>@<date>` a task due on that date. The workbench treats it
//! as opaque.

use fs_components_workbenches::SourceError;
use fs_components_workbenches::calendar::Day;
use fs_store::ObjectId;

/// What a key names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum ItemKey {
    /// A stored event, and which occurrence of it.
    Event {
        id: ObjectId,
        recurrence: Option<String>,
    },
    /// A task, read through the `fs-tasks` interface.
    Task { id: ObjectId, due: Day },
}

fn bad(key: &str) -> SourceError {
    SourceError::not_found(format!("\"{key}\" names nothing."))
}

impl ItemKey {
    /// Read a key.
    pub(super) fn parse(key: &str) -> Result<Self, SourceError> {
        if let Some(rest) = key.strip_prefix("task:") {
            let (id, due) = rest.split_once('@').ok_or_else(|| bad(key))?;
            return Ok(Self::Task {
                id: id.parse().map_err(|_| bad(key))?,
                due: Day::parse(due).ok_or_else(|| bad(key))?,
            });
        }
        let (id, recurrence) = match key.split_once('#') {
            Some((id, recurrence)) => (id, Some(recurrence.to_string())),
            None => (key, None),
        };
        Ok(Self::Event {
            id: id.parse().map_err(|_| bad(key))?,
            recurrence,
        })
    }

    /// The object of an event key; a task cannot be changed here.
    pub(super) fn event_id(key: &str) -> Result<ObjectId, SourceError> {
        match Self::parse(key)? {
            Self::Event { id, .. } => Ok(id),
            Self::Task { .. } => Err(SourceError::unsupported(crate::words::say::TASK_NOTE)),
        }
    }
}
