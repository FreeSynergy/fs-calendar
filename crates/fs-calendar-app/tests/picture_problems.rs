//! A problem line and a refused forced save, as pictures, through the shell's real path. No window.
//!
//! Pattern: Pixel test — skipped without a graphics adapter; pictures kept where `FS_COMP_SHOTS` says.

mod common;

use std::path::PathBuf;

use common::{World, rich, rich_series};
use fs_calendar_app::{labels, screen_widget};
use fs_calendar_core::Event;
use fs_components_app::AppError;
use fs_components_render::RenderError;
use fs_components_workbenches::calendar::CalendarWidget;
use serde_json::json;

fn out(name: &str) -> PathBuf {
    let dir = std::env::var_os("FS_COMP_SHOTS").map_or_else(
        || std::env::temp_dir().join(format!("fs-calendar-app-{}", std::process::id())),
        PathBuf::from,
    );
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

fn read_and_clear(path: &PathBuf) -> Vec<u8> {
    let bytes = std::fs::read(path).unwrap();
    if std::env::var_os("FS_COMP_SHOTS").is_none() {
        std::fs::remove_file(path).unwrap();
        let _ = std::fs::remove_dir(path.parent().unwrap());
    }
    bytes
}

#[test]
fn a_damaged_event_and_a_refused_save_are_drawn() {
    let w = World::berlin();
    let key = w.add_json(rich());
    for (i, (title, marked)) in [("Bad series", false), ("Clinic", true)]
        .into_iter()
        .enumerate()
    {
        let mut bad = rich_series();
        bad["uid"] = json!(format!("urn:uuid:01a1aaf0-08a0-756c-85b2-f092789d000{i}"));
        bad["title"] = json!(title);
        bad["recurrenceRule"] = json!({"@type": "RecurrenceRule", "frequency": "weekly"});
        bad["recurrenceOverrides"] = json!({"2026-11-10T10:00:00": {"duration": "nonsense"}});
        if marked {
            bad["freesynergy.net:sensitive"] = json!(["pd:HealthRecord"]);
        }
        w.add_json(bad);
    }
    let widget = CalendarWidget::new(w.adapter.clone(), labels());
    let app = screen_widget(widget.clone()).unwrap();
    let draw = |name: &str| -> Option<Vec<u8>> {
        let path = out(name);
        match app.picture(1300, 800, &path) {
            Err(AppError::Picture(RenderError::NoGraphics(_))) => None,
            other => {
                other.unwrap();
                Some(read_and_clear(&path))
            }
        }
    };
    let Some(problems) = draw("calendar-problems.png") else {
        return;
    };
    widget.with_state(|s| {
        s.select(&key);
        s.begin_edit();
        s.set_field("start", "2026-11-24 09:00");
    });
    let mut theirs = Event::from_value(w.stored(&key).content).unwrap();
    theirs.start = Some("2026-12-24T10:00:00".into());
    w.runtime
        .block_on(w.repo.update(key.parse().unwrap(), theirs))
        .unwrap();
    widget.with_state(|s| s.save());
    assert!(widget.with_state(|s| s.editing().unwrap().error.is_some()));
    let refused = draw("calendar-refused.png").unwrap();
    assert_ne!(problems, refused);
}
