//! The task program in this process (`LocalTasks`) keeps what the calendar needs of it.

#![cfg(feature = "local")]

mod common;

use std::sync::Arc;

use fs_calendar_tasks::LocalTasks;

#[tokio::test]
async fn local_tasks_behave() {
    let (_store, program) = common::task_program().await;
    let local = Arc::new(LocalTasks::new(Arc::new(program.clone())));
    common::behaves(local.clone(), local, program).await;
}
