//! What the calendar needs of a task program: module map.
//!
//! Pattern: Facade (declarations) — two small traits instead of one big one (interface segregation,
//! ADR 0043): [`TasksInWindow`] to show tasks, [`TaskIntake`] to hand over the tasks of an import. The
//! calendar knows no task type and no crate of `fs-tasks`; the contract is `tasks.capnp` (ADR 0050).

mod error;
mod intake;
mod window;

pub use error::TasksError;
pub use intake::{Forwarded, TASK_TYPE, TaskIntake};
pub use window::{DueTask, TasksInWindow};
