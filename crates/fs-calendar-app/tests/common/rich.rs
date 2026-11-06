//! Events with something in every corner.
//!
//! Pattern: Object mother — what an edit must leave alone.

use serde_json::{Value, json};

/// An event in Berlin with something in every corner the form does not show.
pub fn rich() -> Value {
    json!({
        "@type": "Event",
        "version": "2.0",
        "uid": "urn:uuid:01a1aaf0-08a0-756c-85b2-f092789b0a01",
        "prodId": "-//Elsewhere//EN",
        "created": "2026-10-01T09:00:00Z",
        "updated": "2026-10-01T09:00:00Z",
        "sequence": 7,
        "title": "Kick-off",
        "description": "First line\nSecond line",
        "descriptionContentType": "text/plain",
        "locale": "de",
        "start": "2026-11-24T10:00:00",
        "timeZone": "Europe/Berlin",
        "endTimeZone": "Europe/London",
        "duration": "PT1H30M",
        "status": "tentative",
        "freeBusyStatus": "busy",
        "privacy": "public",
        "priority": 3,
        "keywords": {"work": true},
        "relatedTo": {"urn:uuid:other": {"relation": {"parent": true}}},
        "participants": {
            "p1": {
                "@type": "Participant",
                "name": "NLnet",
                "kind": "group",
                "roles": {"attendee": true},
                "email": "web@nlnet.example",
                "sendTo": {"imip": "mailto:web@nlnet.example"},
                "participationStatus": "needs-action",
                "expectReply": true,
                "freesynergy.net:cardUid": "urn:uuid:person"
            }
        },
        "locations": {
            "l2": {"@type": "Location", "name": "Annex", "coordinates": "geo:52.5,13.4"},
            "l1": {"@type": "Location", "name": "Main hall", "description": "Second floor",
                   "links": {"m": {"@type": "Link", "href": "https://x.example/map"}}}
        },
        "virtualLocations": {"v1": {"@type": "VirtualLocation", "uri": "https://meet.example/a"}},
        "links": {"a": {"@type": "Link", "href": "https://x.example/agenda"}},
        "alerts": {"a1": {"@type": "Alert", "trigger": {"@type": "OffsetTrigger", "offset": "-PT15M"}}},
        "freesynergy.net:source": "https://x.example/1",
        "example.org:anything": {"deep": [1, 2, {"three": true}]}
    })
}

/// A weekly series with an override and an exclusion, and corners of its own.
pub fn rich_series() -> Value {
    let mut event = rich();
    event["uid"] = json!("urn:uuid:01a1aaf0-08a0-756c-85b2-f092789b0a02");
    event["title"] = json!("Jour fixe");
    event["start"] = json!("2026-11-03T10:00:00");
    event["duration"] = json!("PT1H");
    event["recurrenceRule"] = json!({"@type": "RecurrenceRule", "frequency": "weekly", "count": 8});
    event["recurrenceOverrides"] = json!({
        "2026-11-10T10:00:00": {"title": "Moved jour fixe", "start": "2026-11-11T15:00:00"},
        "2026-11-17T10:00:00": {"excluded": true}
    });
    event.as_object_mut().unwrap().remove("endTimeZone");
    event
}
