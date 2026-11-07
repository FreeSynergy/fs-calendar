//! What a question builds is bounded in bytes before it is built: peak memory, not only the answer.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use fs_calendar_core::limits::MAX_ANSWER_BYTES;
use fs_calendar_core::*;
use fs_store::{Store, TursoStore};

struct Counting;
static NOW: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

// SAFETY: forwards to the system allocator and only counts.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let now = NOW.fetch_add(layout.size(), Ordering::Relaxed) + layout.size();
        PEAK.fetch_max(now, Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        NOW.fetch_sub(layout.size(), Ordering::Relaxed);
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

#[tokio::test]
async fn a_long_series_of_a_large_event_never_holds_more_than_the_budget() {
    let store: Arc<dyn Store> = Arc::new(TursoStore::open_in_memory().await.unwrap());
    let r = CalendarRepo::new(store, "https://local.invalid/users/owner");
    let mut e = Event::new("Daily", "2026-01-01T09:00:00", "Etc/UTC", "PT1H");
    e.description = Some("x".repeat(20 * 1024));
    e.recurrence_rule =
        Some(serde_json::from_value(serde_json::json!({"frequency": "daily"})).unwrap());
    r.create(e).await.unwrap();

    let before = NOW.load(Ordering::Relaxed);
    PEAK.store(before, Ordering::Relaxed);
    let window = r
        .window(
            utc("2026-01-01T00:00:00Z").unwrap(),
            utc("2176-01-01T00:00:00Z").unwrap(),
        )
        .await
        .unwrap();
    let peak = PEAK.load(Ordering::Relaxed) - before;
    let held: usize = window
        .instances
        .iter()
        .map(|i| i.occurrence.event.to_value().to_string().len())
        .sum();
    assert!(held <= MAX_ANSWER_BYTES, "{held} bytes of instances");
    assert!(
        window.damaged.iter().any(|d| d.reason.contains("cut")),
        "{:?}",
        window.damaged
    );
    assert!(
        peak < 4 * MAX_ANSWER_BYTES,
        "peak {peak} bytes for an answer of {held}"
    );
}
