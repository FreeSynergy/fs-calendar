//! Small data types of the repository API.
//!
//! Pattern: Value objects — plain data for query results and import reports.

use fs_objects::ObjectId;

use fs_tasks_core::TaskEntry;

use crate::recur::Occurrence;

/// An event instance with the object it belongs to.
#[derive(Clone, Debug, PartialEq)]
pub struct Instance {
    /// The stored object the instance belongs to.
    pub object: ObjectId,
    /// The concrete instance.
    pub occurrence: Occurrence,
}

/// One event, or one instance of it, that a question could not answer for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Damaged {
    /// The stored event.
    pub object: ObjectId,
    /// The override key of the instance, if only that instance is damaged.
    pub key: Option<String>,
    /// What went wrong, or that the answer was cut.
    pub reason: String,
    /// The answer was cut at a limit (instances per event or bytes of the answer): nothing is damaged,
    /// something is missing. The reason starts with `cut: `.
    pub cut: bool,
}

/// The answer to a time window: the instances, and what could not be answered for. One damaged event
/// or override costs only itself; the calendar of everything else stays.
#[derive(Clone, Debug, Default)]
pub struct Window {
    /// Instances overlapping the window, ordered by start.
    pub instances: Vec<Instance>,
    /// Events or instances left out, with the reason.
    pub damaged: Vec<Damaged>,
}

/// Events and due tasks of one time window, for a calendar view.
#[derive(Clone, Debug, Default)]
pub struct Agenda {
    /// Event instances overlapping the window, series expanded, ordered by start.
    pub events: Vec<Instance>,
    /// Tasks due in the window, from the `fs-tasks` interface (empty if none is attached).
    pub tasks: Vec<TaskEntry>,
    /// Events or instances that could not be answered for, with the reason (see [`Window`]).
    pub damaged: Vec<Damaged>,
}

/// Result of an import.
#[derive(Clone, Debug, Default)]
pub struct ImportReport {
    /// Objects that were created.
    pub created: Vec<ObjectId>,
    /// `uid`s that already existed (not overwritten).
    pub skipped: Vec<String>,
    /// Entries that could not be converted or validated.
    pub failed: Vec<String>,
    /// What the 1.0 -> 2.0 upgrade changed, and tasks left out because no task program is attached.
    pub notes: Vec<String>,
}
