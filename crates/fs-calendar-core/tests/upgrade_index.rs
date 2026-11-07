//! Events written by v0.4.1 have no `page_key` in their index. The fixture `fixtures/v041.db` (with its
//! `-wal`) was written by the code of tag v0.4.1, six events through `CalendarRepo::create` (one a daily
//! series of three), not derived from the current code. Opening it must lose nothing silently, and what
//! cannot be repaired is said.

mod common;

use std::path::Path;
use std::sync::Arc;

use common::{ME, t};
use fs_calendar_core::{CalendarRepo, Error};
use fs_objects::{Store, TursoStore};

fn copy_fixture(to: &Path) -> std::path::PathBuf {
    let from = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    for name in ["v041.db", "v041.db-wal"] {
        std::fs::copy(from.join(name), to.join(name)).unwrap();
    }
    to.join("v041.db")
}

async fn open(path: &Path) -> (CalendarRepo, Arc<dyn Store>) {
    let store: Arc<dyn Store> = Arc::new(TursoStore::open(path).await.unwrap());
    (CalendarRepo::new(store.clone(), ME), store)
}

#[tokio::test]
async fn events_without_a_place_are_not_left_out_silently() {
    let dir = tempfile::tempdir().unwrap();
    let (repo, _store) = open(&copy_fixture(dir.path())).await;
    assert_eq!(repo.list_events().await.unwrap().len(), 6);

    let (from, to) = (t("2026-12-01T00:00:00Z"), t("2027-01-01T00:00:00Z"));
    let page = repo.list_page(None, 100).await;
    assert!(matches!(page, Err(Error::Stored(_))), "{page:?}");
    assert!(page.unwrap_err().to_string().contains("upgrade_index"));
    for asked in [
        repo.window(from, to).await.map(|_| ()),
        repo.occurrences(from, to).await.map(|_| ()),
    ] {
        assert!(matches!(asked, Err(Error::Stored(_))), "{asked:?}");
    }
}

#[tokio::test]
async fn the_upgrade_gives_them_a_place_and_changes_nothing_twice() {
    let dir = tempfile::tempdir().unwrap();
    let (repo, _store) = open(&copy_fixture(dir.path())).await;
    let report = repo.upgrade_index().await.unwrap();
    assert_eq!(
        (report.rewritten, report.failed.len()),
        (6, 0),
        "{report:?}"
    );
    assert_eq!(repo.upgrade_index().await.unwrap().rewritten, 0);

    let (from, to) = (t("2026-12-01T00:00:00Z"), t("2027-01-01T00:00:00Z"));
    let (mut seen, mut after) = (vec![], None);
    loop {
        let page = repo.list_page(after, 4).await.unwrap();
        seen.extend(page.items.iter().map(|o| o.envelope.name.clone()));
        after = page.items.last().map(|o| o.envelope.id);
        if !page.more {
            break;
        }
    }
    seen.sort();
    assert_eq!(seen, (0..6).map(|i| format!("Old {i}")).collect::<Vec<_>>());
    let window = repo.window(from, to).await.unwrap();
    assert_eq!((window.instances.len(), window.damaged.len()), (8, 0));
    assert_eq!(repo.occurrences(from, to).await.unwrap().len(), 8);
}

#[tokio::test]
async fn one_card_that_cannot_be_repaired_is_named_and_stops_no_other() {
    let dir = tempfile::tempdir().unwrap();
    let (repo, store) = open(&copy_fixture(dir.path())).await;
    let mut broken = repo.list_events().await.unwrap().remove(0);
    broken.content["duration"] = "nonsense".into();
    store.put(broken.clone(), ME).await.unwrap();
    let report = repo.upgrade_index().await.unwrap();
    assert_eq!(report.rewritten, 5, "{report:?}");
    assert_eq!(report.failed.len(), 1);
    assert!(report.failed[0].starts_with(&broken.envelope.id.to_string()));
    // The one left stays without a place, and a page says so.
    assert!(matches!(
        repo.list_page(None, 10).await,
        Err(Error::Stored(_))
    ));
}
