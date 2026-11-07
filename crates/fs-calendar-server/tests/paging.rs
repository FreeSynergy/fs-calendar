//! `Items.list` over a real socket: pages cut at a key, with many page sizes and data changing
//! between two pages.

mod common;

use common::{create, items_of, read_pages, set_id, with_page_size};
use fs_interfaces::rpc::{INVALID_REQUEST, Problem};

const UIDS: usize = 14;

async fn fill(items: &fs_interfaces::events_capnp::items::Client) {
    for i in 0..UIDS {
        // Starts run against the order of creation, so an order by start would differ from the key.
        let start = format!("2027-03-{:02}T10:00:00", 20 - i);
        create(items, &format!("u-{i:02}"), &format!("T{i:02}"), &start).await;
    }
}

#[tokio::test(flavor = "current_thread")]
async fn every_page_size_gives_every_event_once_in_the_same_order() {
    let reference = std::rc::Rc::new(std::cell::RefCell::new(None::<Vec<String>>));
    for size in [1, 2, 3, 5, 7, 13, 14, 100] {
        let reference = reference.clone();
        with_page_size(size, |root| async move {
            let items = items_of(&root).await;
            fill(&items).await;
            let uids = read_pages(&items, |_| async {}).await;
            let mut sorted = uids.clone();
            sorted.sort();
            sorted.dedup();
            assert_eq!(sorted.len(), UIDS, "size {size}: {uids:?}");
            let mut first = reference.borrow_mut();
            match first.as_ref() {
                None => *first = Some(uids),
                Some(first) => assert_eq!(&uids, first, "size {size}"),
            }
        })
        .await;
    }
}

#[tokio::test(flavor = "current_thread")]
async fn an_event_deleted_or_added_between_two_pages_moves_no_boundary() {
    with_page_size(4, |root| async move {
        let items = items_of(&root).await;
        fill(&items).await;
        let all = read_pages(&items, |_| async {}).await;

        let seen = read_pages(&items, |first_page_ids| {
            let items = items.clone();
            async move {
                // Two events of the page go, and one arrives that sorts anywhere.
                for id in &first_page_ids[..2] {
                    let mut delete = items.delete_request();
                    set_id(delete.get().init_id(), *id);
                    delete.send().promise.await.unwrap();
                }
                create(&items, "late", "Late", "2020-01-01T10:00:00").await;
            }
        })
        .await;
        // What came after the first page is exactly what was there, nothing twice and nothing lost;
        // the new event stands behind the boundary or, if its id sorts before it, is not seen.
        let rest: Vec<_> = seen[4..].iter().filter(|u| *u != "late").cloned().collect();
        assert_eq!(rest, all[4..], "{seen:?}");
        assert!(
            seen.iter().filter(|u| *u == "late").count() <= 1,
            "{seen:?}"
        );
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn a_cursor_that_is_not_ours_is_an_invalid_request() {
    with_page_size(4, |root| async move {
        let items = items_of(&root).await;
        fill(&items).await;
        let position = [&[1u8][..], &2u64.to_le_bytes()].concat();
        for cursor in [&b"garbage"[..], &[0xff], &[2], &position, &[2, b'x', b'y']] {
            let mut call = items.list_request();
            call.get().init_cursor().set_opaque(cursor);
            let error = call.send().promise.await.err().expect("refused");
            let problem = Problem::from_error(&error).unwrap();
            assert_eq!(problem.problem_type, INVALID_REQUEST, "{cursor:?}");
        }
    })
    .await;
}
