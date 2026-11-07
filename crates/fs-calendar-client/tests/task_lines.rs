//! The tasks of a day from the real tasks server, read through `TaskLines`.

mod common;

use chrono::{TimeZone, Utc};
use common::TasksServed;
use fs_calendar_client::TaskLines;
use serde_json::json;

fn day(d: u32) -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 12, d, 0, 0, 0).unwrap()
}

#[tokio::test]
async fn only_the_tasks_due_in_the_window_come_with_their_content() {
    let served = TasksServed::start().await;
    let inside = served
        .add("Send the minutes", Some("2026-12-10T17:00:00"))
        .await;
    served.add("Later", Some("2027-05-01T09:00:00")).await;
    served.add("Whenever", None).await;
    let lines = served.client().due_between(day(9), day(12)).await.unwrap();
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert_eq!(lines[0].id.to_string(), inside);
    assert_eq!(lines[0].name, "Send the minutes");
    assert_eq!(lines[0].content["due"], json!("2026-12-10T17:00:00"));
    assert_eq!(lines[0].content["timeZone"], json!("Europe/Berlin"));
}

#[tokio::test]
async fn more_tasks_than_a_page_come_whole() {
    let served = TasksServed::start().await;
    for n in 0..230 {
        served
            .add(
                &format!("T{n}"),
                Some(&format!("2026-12-{:02}T10:00:00", 1 + n % 28)),
            )
            .await;
    }
    let lines = served.client().due_between(day(1), day(30)).await.unwrap();
    assert_eq!(lines.len(), 230);
}

#[tokio::test]
async fn a_tasks_server_that_is_gone_is_said_in_words() {
    let mut served = TasksServed::start().await;
    let client = served.client();
    served.stop();
    let err = client.due_between(day(1), day(2)).await.unwrap_err();
    assert!(err.to_string().contains("tasks"), "{err}");
}
