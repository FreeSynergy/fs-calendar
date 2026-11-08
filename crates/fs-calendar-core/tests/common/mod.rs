//! Shared helpers of the integration tests.

#![allow(dead_code)] // each test file uses a different subset

use std::sync::Arc;

use chrono::{DateTime, Utc};
use fs_calendar_core::{CalendarRepo, utc};
use fs_objects::{Store, TursoStore};

pub mod fake_tasks;

pub const ME: &str = "https://local.invalid/users/owner";
pub const DEMO: &str = "/srv/FreeSynergy/fs-documentation/temp/demo-data/events.json";

pub const ICS: &str = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//x//y//EN\r\nBEGIN:VEVENT\r\nUID:ev-1\r\nDTSTAMP:20261101T000000Z\r\nDTSTART;TZID=Europe/Berlin:20261124T100000\r\nDTEND;TZID=Europe/Berlin:20261124T113000\r\nSUMMARY:Jour fixe\r\nDESCRIPTION:Hello\\, world\r\nLOCATION:Berlin\r\nRRULE:FREQ=WEEKLY;COUNT=4;BYDAY=TU\r\nEXDATE;TZID=Europe/Berlin:20261201T100000\r\nORGANIZER;CN=Me:mailto:me@example.org\r\nATTENDEE;CN=Ben;PARTSTAT=NEEDS-ACTION;RSVP=TRUE:mailto:ben@example.org\r\nSTATUS:CONFIRMED\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nUID:ev-2\r\nDTSTAMP:20261101T000000Z\r\nDTSTART:20261130T090000Z\r\nDURATION:PT2H\r\nSUMMARY:Second\r\nEND:VEVENT\r\nBEGIN:VTODO\r\nUID:todo-1\r\nDTSTAMP:20261101T000000Z\r\nDUE:20261112T170000Z\r\nSUMMARY:Minutes\r\nSTATUS:NEEDS-ACTION\r\nPERCENT-COMPLETE:10\r\nEND:VTODO\r\nEND:VCALENDAR\r\n";

/// A repository over a fresh in-memory store.
pub async fn repo() -> CalendarRepo {
    setup().await.0
}

/// The repository and a handle to its store (for undo, log and count checks).
pub async fn setup() -> (CalendarRepo, Arc<dyn Store>) {
    let store: Arc<dyn Store> = Arc::new(TursoStore::open_in_memory().await.unwrap());
    (CalendarRepo::new(store.clone(), ME), store)
}

/// Parse a UTC timestamp.
pub fn t(s: &str) -> DateTime<Utc> {
    utc(s).unwrap()
}
