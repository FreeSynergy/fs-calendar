//! The command line.
//!
//! Pattern: Table of inputs and answers.

use std::path::PathBuf;

use fs_calendar_app::args::parse;
use fs_calendar_app::{shown_zone, zone_from};
use fs_components_workbenches::calendar::{Day, ViewRange};

fn args(line: &str) -> Vec<String> {
    line.split_whitespace().map(String::from).collect()
}

#[test]
fn all_options_are_read_and_nothing_is_the_default() {
    let got = parse(&args(
        "--store a.db --import b.ics --export c.ics --snapshot d.png --size 800x600 --date 2026-11-06 --zone Asia/Tokyo --view week --week-start sunday",
    ))
    .unwrap();
    assert_eq!(got.store, Some(PathBuf::from("a.db")));
    assert_eq!(got.import, Some(PathBuf::from("b.ics")));
    assert_eq!(got.export, Some(PathBuf::from("c.ics")));
    assert_eq!(got.snapshot, Some(PathBuf::from("d.png")));
    assert_eq!(got.size, (800, 600));
    assert_eq!(got.date, Day::parse("2026-11-06"));
    assert_eq!(got.zone.as_deref(), Some("Asia/Tokyo"));
    assert_eq!(got.view, Some(ViewRange::Week));
    assert_eq!(got.week_start, 6);
    let none = parse(&[]).unwrap();
    assert_eq!(none.store, None, "in memory");
    assert_eq!(none.date, None, "the real clock");
    assert_eq!(none.week_start, 0);
}

#[test]
fn mistakes_are_said() {
    assert!(parse(&args("--nope")).unwrap_err().contains("unknown"));
    assert!(
        parse(&args("--store"))
            .unwrap_err()
            .contains("needs a value")
    );
    for bad in ["10", "axb", "0x5", "5x0"] {
        assert!(
            parse(&args(&format!("--size {bad}")))
                .unwrap_err()
                .contains("WIDTHxHEIGHT")
        );
    }
    assert!(
        parse(&args("--date 2026-02-30"))
            .unwrap_err()
            .contains("YYYY-MM-DD")
    );
    assert!(
        parse(&args("--view year"))
            .unwrap_err()
            .contains("month, week")
    );
    assert!(
        parse(&args("--week-start friday"))
            .unwrap_err()
            .contains("monday")
    );
    assert!(
        parse(&args("--export-sensitive a"))
            .unwrap_err()
            .contains("needs --export")
    );
    assert!(shown_zone(Some("Mars/Olympus")).is_err());
    assert_eq!(
        shown_zone(Some("Asia/Tokyo")).unwrap().0.name(),
        "Asia/Tokyo"
    );
    assert!(
        parse(&args("--date 9001-01-01"))
            .unwrap_err()
            .contains("years")
    );
    assert!(
        parse(&args("--date 0000-01-01"))
            .unwrap_err()
            .contains("years")
    );
}

#[test]
fn a_tz_that_names_nothing_is_said_and_a_good_one_is_used() {
    let berlin = Some("Europe/Berlin".to_string());
    let (zone, note) = zone_from(None, Some("Mars/Olympus"), berlin.clone()).unwrap();
    assert_eq!(zone.name(), "Etc/UTC");
    assert!(note.unwrap().contains("Mars/Olympus"));
    let (zone, note) = zone_from(None, Some(":Asia/Tokyo"), berlin.clone()).unwrap();
    assert_eq!((zone.name(), note), ("Asia/Tokyo", None));
    let (zone, note) = zone_from(None, Some(""), berlin).unwrap();
    assert_eq!((zone.name(), note), ("Europe/Berlin", None));
    let (zone, note) = zone_from(None, None, None).unwrap();
    assert_eq!(zone.name(), "Etc/UTC");
    assert!(note.unwrap().contains("UTC"));
    assert!(zone_from(Some("Mars/x"), Some("Asia/Tokyo"), None).is_err());
}
