//! Event model: lossless round trip, validation, 1.0 -> 2.0 upgrade.

mod common;

use common::DEMO;
use fs_calendar_core::validate::{issues, validate};
use fs_calendar_core::*;
use serde_json::{Value, json};

#[test]
fn demo_events_roundtrip_losslessly_and_validate() {
    let items: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(DEMO).unwrap()).unwrap();
    assert_eq!(items.len(), 5);
    for item in items {
        let obj = Event::from_value(item.clone()).unwrap();
        validate(&obj).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(obj.to_value(), item);
        // already 2.0: the upgrade changes nothing
        let up = upgrade(item.clone());
        assert_eq!(up.value, item);
        assert!(up.notes.is_empty(), "{:?}", up.notes);
    }
}

#[test]
fn unknown_and_vendor_properties_survive() {
    let v = json!({"@type":"Event","version":"2.0","uid":"u","start":"2026-01-01T10:00:00","title":"x",
        "locations":{"l":{"name":"Here","example.com:geo":[1,2]}},"alerts":{"a":{"trigger":{"offset":"-PT5M"}}},
        "freesynergy.net:taskStatus":"x","future":{"a":1},
        "participants":{"p":{"name":"N","sendTo":{"imip":"mailto:n@x.org"},"vendor:x":true}}});
    let e = Event::from_value(v.clone()).unwrap();
    assert_eq!(e.to_value(), v);
}

#[test]
fn validation_rules() {
    let ok = Event::new("A", "2026-11-24T10:00:00", "Europe/Berlin", "PT1H");
    assert!(issues(&ok).is_empty());
    let mut bad = ok.clone();
    bad.version = Some("1.0".into());
    bad.start = Some("2026-11-24T10:00:00Z".into());
    bad.time_zone = Some("/Custom/Zone".into());
    bad.duration = Some("1h".into());
    bad.status = Some("maybe".into());
    bad.recurrence_rule = Some(
        serde_json::from_value(
            json!({"frequency":"daily","count":3,"until":"2026-12-01T00:00:00"}),
        )
        .unwrap(),
    );
    bad.recurrence_overrides
        .insert("not-a-date".into(), json!({}));
    bad.extra.insert("recurrenceRules".into(), json!([]));
    bad.add_invitee("B", "b@x.org");
    bad.participants.get_mut("p1").unwrap().participation_status = Some("yes".into());
    let paths: Vec<_> = issues(&bad).into_iter().map(|i| i.path).collect();
    for p in [
        "/version",
        "/start",
        "/timeZone",
        "/duration",
        "/status",
        "/recurrenceRule",
        "/recurrenceOverrides/not-a-date",
        "/recurrenceRules",
        "/participants/p1/participationStatus",
    ] {
        assert!(paths.iter().any(|x| x == p), "{p} in {paths:?}");
    }
    let mut not_an_event = ok.clone();
    not_an_event.type_ = Some("Task".into());
    let paths: Vec<_> = issues(&not_an_event).into_iter().map(|i| i.path).collect();
    assert!(
        paths.contains(&"/@type".to_string()),
        "tasks belong to fs-tasks"
    );
}

#[test]
fn upgrade_from_rfc_8984() {
    let v1 = json!({
        "@type": "Event", "uid": "e1", "updated": "2026-01-01T00:00:00Z", "title": "Old",
        "start": "2026-11-03T10:00:00.000", "timeZone": "Europe/Berlin", "duration": "PT30M",
        "recurrenceRules": [{"frequency":"weekly","count":12}, {"frequency":"monthly"}],
        "excludedRecurrenceRules": [{"frequency":"weekly","byDay":[{"day":"fr"}]}],
        "timeZones": {"Custom": {"@type":"TimeZone","tzId":"Custom"}},
        "recurrenceOverrides": {"2026-11-17T10:00:00": {"excluded": true}},
        "locations": {"l1": {"name":"Room","timeZone":"Europe/Berlin","relativeTo":"start"}},
        "participants": {"p1": {"name":"B","language":"de","locationId":"l1","sendTo":{"imip":"mailto:b@x.org"}}},
        "links": {"k": {"href":"https://x.org","cid":"abc"}}
    });
    let up = upgrade(v1.clone());
    let v = &up.value;
    assert_eq!(v["version"], "2.0");
    assert_eq!(
        v["recurrenceRule"],
        json!({"frequency":"weekly","count":12})
    );
    assert!(
        v.get("recurrenceRules").is_none()
            && v.get("timeZones").is_none()
            && v.get("excludedRecurrenceRules").is_none()
    );
    assert_eq!(v["start"], "2026-11-03T10:00:00");
    let legacy = &v["freesynergy.net:legacy"];
    assert_eq!(legacy["recurrenceRules"], json!([{"frequency":"monthly"}]));
    assert!(legacy["timeZones"].is_object() && legacy["excludedRecurrenceRules"].is_array());
    assert_eq!(legacy["locations/l1/timeZone"], "Europe/Berlin");
    assert_eq!(legacy["participants/p1/language"], "de");
    assert_eq!(legacy["links/k/cid"], "abc");
    assert_eq!(legacy["startOriginal"], "2026-11-03T10:00:00.000");
    assert!(v["locations"]["l1"].get("timeZone").is_none());
    assert_eq!(
        v["participants"]["p1"]["sendTo"]["imip"], "mailto:b@x.org",
        "reserved props stay"
    );
    assert!(
        up.notes
            .iter()
            .any(|n| n.contains("further recurrence rule"))
    );
    let obj = Event::from_value(v.clone()).unwrap();
    validate(&obj).unwrap();
    // idempotent
    assert_eq!(upgrade(up.value.clone()).value, up.value);
    // Z suffix and custom zone
    let up = upgrade(json!({"@type":"Event","uid":"t","start":"2026-11-12T17:00:00Z"}));
    assert_eq!(
        (up.value["start"].as_str(), up.value["timeZone"].as_str()),
        (Some("2026-11-12T17:00:00"), Some("Etc/UTC"))
    );
    let up = upgrade(
        json!({"@type":"Event","uid":"c","start":"2026-01-01T10:00:00","timeZone":"/Custom/Zone"}),
    );
    assert!(
        up.value.get("timeZone").is_none()
            && up.value["freesynergy.net:legacy"]["timeZone"] == "/Custom/Zone"
    );
    // Group
    let up = upgrade(
        json!({"@type":"Group","entries":[{"@type":"Event","uid":"a","recurrenceRules":[{"frequency":"daily"}]}]}),
    );
    assert_eq!(
        up.value["entries"][0]["recurrenceRule"]["frequency"],
        "daily"
    );
}
