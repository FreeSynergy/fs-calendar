//! The candidates are paged by a key the store sorts and compares the same way: no event is lost.

mod common;

use common::{repo, t};
use fs_calendar_core::*;

#[tokio::test]
async fn events_whose_uids_differ_in_case_are_all_found() {
    let r = repo().await;
    for uid in ["a-one", "B-two", "c-three", "Ab-four", "aa-five"] {
        let mut e = Event::new(uid, "2026-12-01T10:00:00", "Etc/UTC", "PT1H");
        e.uid = uid.into();
        r.create(e).await.unwrap();
    }
    let w = r
        .window(t("2026-12-01T00:00:00Z"), t("2026-12-02T00:00:00Z"))
        .await
        .unwrap();
    let mut titles: Vec<_> = w
        .instances
        .iter()
        .map(|i| i.occurrence.event.display_title())
        .collect();
    titles.sort();
    assert_eq!(
        titles,
        ["Ab-four", "B-two", "a-one", "aa-five", "c-three"],
        "{:?}",
        w.damaged
    );
}

#[tokio::test]
async fn many_events_with_mixed_uids_are_found_over_several_pages() {
    let r = repo().await;
    for i in 0..300 {
        let uid = if i % 3 == 0 {
            format!("Mixed-{i}")
        } else {
            format!("mixed-{i}")
        };
        let mut e = Event::new(&uid, "2026-12-01T10:00:00", "Etc/UTC", "PT1H");
        e.uid = uid;
        r.create(e).await.unwrap();
    }
    let w = r
        .window(t("2026-12-01T00:00:00Z"), t("2026-12-02T00:00:00Z"))
        .await
        .unwrap();
    assert_eq!(w.instances.len(), 300, "{:?}", w.damaged);
}
