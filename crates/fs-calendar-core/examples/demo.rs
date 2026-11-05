//! Import the demo events, list the next 30 days, accept an invitation, export as iCalendar.
//!
//! cargo run -p fs-calendar-core --example demo -- [events.json] [YYYY-MM-DD "today"]

use std::sync::Arc;

use fs_calendar_core::{CalendarRepo, Response, time::TZDB_VERSION, utc};
use fs_store::{Store, TursoStore};

const DEMO: &str = "/srv/FreeSynergy/fs-documentation/temp/demo-data/events.json";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let path = args.next().unwrap_or_else(|| DEMO.into());
    let today = args.next().map(|d| format!("{d}T00:00:00Z")).unwrap_or_else(fs_calendar_core::now_utc);
    let now = utc(&today)?;

    let store: Arc<dyn Store> = Arc::new(TursoStore::open_in_memory().await?);
    let repo = CalendarRepo::new(store, "https://local.invalid/users/owner");
    let rep = repo.import_jscalendar(&std::fs::read_to_string(path)?).await?;
    println!("imported {} objects ({} skipped, {} failed); tzdb {TZDB_VERSION}", rep.created.len(), rep.skipped.len(), rep.failed.len());

    println!("\nnext 30 days from {now}:");
    let next = repo.upcoming(now, 30).await?;
    for i in &next {
        println!("  {}  {}", i.occurrence.start_utc.format("%a %Y-%m-%d %H:%MZ"), i.occurrence.event.display_title());
    }

    if let Some(first) = next.first() {
        let ev = repo.get_event(first.object).await?.unwrap();
        let (key, p) = ev.participants.iter().find(|(_, p)| p.participation_status.as_deref() == Some("needs-action")).ok_or("no open invitation")?;
        println!("\n{} answers the invitation to {:?}: Accept", p.name.as_deref().unwrap_or(key), ev.title.as_deref().unwrap_or(""));
        let email = p.email.clone().unwrap_or_else(|| key.clone());
        repo.respond(first.object, &email, Response::Accept, Some("Looking forward to it")).await?;
        for e in repo.invitation_log(first.object).await? {
            println!("  log: {} by {} ({:?})", e.activity, e.participant, e.comment);
        }
        println!("\niCalendar:\n{}", repo.export_ics(&[first.object]).await?);
    }
    Ok(())
}
