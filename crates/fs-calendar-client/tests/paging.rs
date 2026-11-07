//! `list_events` reads every page the server cuts, in the server's order.

mod common;

use common::{Served, event};
use fs_calendar_core::Calendar;

#[tokio::test]
async fn more_than_one_page_of_events_comes_whole_and_in_order() {
    let served = Served::start().await;
    let c = served.client();
    let total = 230;
    for n in 0..total {
        let start = format!("2026-{:02}-{:02}T09:00:00", 1 + n / 28, 1 + n % 28);
        Calendar::create(
            served.repo.as_ref(),
            event(&format!("p{n}"), &format!("E{n}"), &start),
        )
        .await
        .unwrap();
    }
    let listed = c.list_events().await.unwrap();
    assert_eq!(listed.len(), total);
    let direct = Calendar::list_events(served.repo.as_ref()).await.unwrap();
    let ids = |l: &[fs_store::StoredObject]| l.iter().map(|o| o.envelope.id).collect::<Vec<_>>();
    assert_eq!(
        ids(&listed),
        ids(&direct),
        "the same events in the same order, none twice"
    );
}

#[tokio::test]
async fn an_empty_calendar_is_an_empty_list() {
    let served = Served::start().await;
    assert!(served.client().list_events().await.unwrap().is_empty());
}
