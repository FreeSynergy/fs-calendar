//! Durations, zones, DST rules and the tz database selection (ADR 0026).

use chrono::Duration;
use fs_calendar_core::time::{
    BundledTzdb, Dur, SystemTzdb, TzSource, Zone, ZoneSource, fmt_utc, newest, parse_local,
    tzdb_source, tzdb_version,
};

#[test]
fn durations() {
    assert_eq!(
        Dur::parse("PT1H").unwrap(),
        Dur {
            days: 0,
            secs: 3600
        }
    );
    assert_eq!(
        Dur::parse("P1DT2H30M").unwrap(),
        Dur {
            days: 1,
            secs: 9000
        }
    );
    assert_eq!(Dur::parse("P2W").unwrap(), Dur { days: 14, secs: 0 });
    for bad in ["", "P", "PT", "1H", "PT1", "P1H", "PTH", "P-1D"] {
        assert!(Dur::parse(bad).is_err(), "{bad}");
    }
}

#[test]
fn zones_and_dst() {
    let berlin = Zone::parse("Europe/Berlin").unwrap();
    let l = parse_local("2026-11-24T10:00:00").unwrap();
    assert_eq!(fmt_utc(berlin.to_utc(l)), "2026-11-24T09:00:00Z");
    let l = parse_local("2026-07-01T10:00:00").unwrap();
    assert_eq!(fmt_utc(berlin.to_utc(l)), "2026-07-01T08:00:00Z");
    // gap: 2026-03-29 02:30 does not exist in Berlin; offset before the gap is +1 -> 01:30Z
    let gap = parse_local("2026-03-29T02:30:00").unwrap();
    assert_eq!(fmt_utc(berlin.to_utc(gap)), "2026-03-29T01:30:00Z");
    // overlap: 2026-10-25 02:30 happens twice; first is CEST (+2) -> 00:30Z
    let dup = parse_local("2026-10-25T02:30:00").unwrap();
    assert_eq!(fmt_utc(berlin.to_utc(dup)), "2026-10-25T00:30:00Z");
    // a nominal day across the DST change is 25 h
    let start = parse_local("2026-10-24T12:00:00").unwrap();
    let end = Dur::parse("P1D").unwrap().end_utc(&berlin, start);
    assert_eq!(end - berlin.to_utc(start), Duration::hours(24 + 1));
}

#[test]
fn rejects_non_iana() {
    assert!(Zone::parse("/Custom/Zone").is_err());
    assert!(Zone::parse("Mars/Olympus").is_err());
    assert!(parse_local("2026-11-24T10:00:00Z").is_err());
    assert!(parse_local("2026-11-24T10:00:00.5").is_err());
    assert_ne!(tzdb_version(), "");
}

#[test]
fn tzdb_selection_prefers_the_newest() {
    let dir = std::env::temp_dir().join(format!("fs-cal-tzdb-{}", std::process::id()));
    let fake = |v: &str| {
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("tzdata.zi"),
            format!("# version {v}\nZ Etc/UTC 0 - UTC\n"),
        )
        .unwrap();
        SystemTzdb::from_dir(&dir).unwrap()
    };
    let newer = newest(Some(fake("2999z")));
    assert_eq!(
        (
            newer.version(),
            matches!(newer.source(), TzSource::System(_))
        ),
        ("2999z", true)
    );
    let older = newest(Some(fake("2001a")));
    assert_eq!(older.version(), BundledTzdb::new().version());
    assert_eq!(*older.source(), TzSource::Bundled);
    assert_eq!(newest(None).version(), BundledTzdb::new().version());
    // no version information -> cannot be recorded -> not used
    std::fs::remove_file(dir.join("tzdata.zi")).unwrap();
    assert!(SystemTzdb::from_dir(&dir).is_none());
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn active_tzdb_is_at_least_the_bundled_one_and_agrees_with_it() {
    let v = tzdb_version();
    assert!(
        v.len() == 5 && v[..4].chars().all(|c| c.is_ascii_digit()),
        "{v}"
    );
    assert!(v >= BundledTzdb::new().version());
    eprintln!("tzdb in use: {v} from {:?}", tzdb_source());
    let bundled = BundledTzdb::new();
    for name in [
        "Europe/Berlin",
        "America/New_York",
        "Asia/Kolkata",
        "Australia/Lord_Howe",
        "UTC",
        "Etc/UTC",
    ] {
        let (a, b) = (Zone::parse(name).unwrap(), bundled.zone(name).unwrap());
        for l in [
            "2026-03-29T02:30:00",
            "2026-10-25T02:30:00",
            "2026-07-01T12:00:00",
            "2030-01-01T00:00:00",
        ] {
            let l = parse_local(l).unwrap();
            assert_eq!(a.to_utc(l), b.to_utc(l), "{name} {l}");
            assert_eq!(a.to_local(a.to_utc(l)), b.to_local(b.to_utc(l)));
        }
    }
}

#[test]
fn zone_ids_are_plain_names_only() {
    for bad in [
        "../etc/passwd",
        "Europe//Berlin",
        "Europe/Ber lin",
        "",
        "Europe/../Berlin",
        "\\x",
        "./UTC",
    ] {
        assert!(Zone::parse(bad).is_err(), "{bad:?}");
    }
    assert_eq!(Zone::parse("Etc/UTC").unwrap(), Zone::Utc);
    assert_eq!(
        Zone::parse("Europe/Berlin").unwrap().name(),
        "Europe/Berlin"
    );
}

#[test]
fn a_zone_can_be_looked_up_in_a_chosen_source() {
    let bundled = BundledTzdb::new();
    let z = Zone::parse_in(&bundled, "Europe/Berlin").unwrap();
    assert_eq!(z, Zone::parse("Europe/Berlin").unwrap());
    assert!(Zone::parse_in(&bundled, "../x").is_err());
}
