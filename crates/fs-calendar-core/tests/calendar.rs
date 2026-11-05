use std::sync::Arc;

use fs_calendar_core::validate::{issues, validate};
use fs_calendar_core::*;
use fs_store::{ActivityFilter, ActivityKind, Store, TursoStore};
use serde_json::{Value, json};

const ME: &str = "https://local.invalid/users/owner";
const DEMO: &str = "/srv/FreeSynergy/fs-documentation/temp/demo-data/events.json";

async fn repo() -> CalendarRepo {
    let store: Arc<dyn Store> = Arc::new(TursoStore::open_in_memory().await.unwrap());
    CalendarRepo::new(store, ME)
}

fn t(s: &str) -> chrono::DateTime<chrono::Utc> {
    utc(s).unwrap()
}

#[test]
fn demo_events_roundtrip_losslessly_and_validate() {
    let items: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(DEMO).unwrap()).unwrap();
    assert_eq!(items.len(), 5);
    for item in items {
        let obj = CalendarObject::from_value(item.clone()).unwrap();
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
    let tk = json!({"@type":"Task","version":"2.0","uid":"t","due":"2026-01-01T10:00:00","freesynergy.net:taskStatus":"waiting","percentComplete":10});
    assert_eq!(Task::from_value(tk.clone()).unwrap().to_value(), tk);
}

#[test]
fn validation_rules() {
    let ok = Event::new("A", "2026-11-24T10:00:00", "Europe/Berlin", "PT1H");
    assert!(issues(&ok.clone().into()).is_empty());
    let mut bad = ok.clone();
    bad.version = Some("1.0".into());
    bad.start = Some("2026-11-24T10:00:00Z".into());
    bad.time_zone = Some("/Custom/Zone".into());
    bad.duration = Some("1h".into());
    bad.status = Some("maybe".into());
    bad.recurrence_rule = Some(serde_json::from_value(json!({"frequency":"daily","count":3,"until":"2026-12-01T00:00:00"})).unwrap());
    bad.recurrence_overrides.insert("not-a-date".into(), json!({}));
    bad.extra.insert("recurrenceRules".into(), json!([]));
    bad.add_invitee("B", "b@x.org");
    bad.participants.get_mut("p1").unwrap().participation_status = Some("yes".into());
    let paths: Vec<_> = issues(&bad.into()).into_iter().map(|i| i.path).collect();
    for p in ["/version", "/start", "/timeZone", "/duration", "/status", "/recurrenceRule", "/recurrenceOverrides/not-a-date", "/recurrenceRules", "/participants/p1/participationStatus"] {
        assert!(paths.iter().any(|x| x == p), "{p} in {paths:?}");
    }
    let mut task = Task::new("T");
    task.percent_complete = Some(101);
    task.progress = Some("sleeping".into());
    let paths: Vec<_> = issues(&task.into()).into_iter().map(|i| i.path).collect();
    assert!(paths.contains(&"/percentComplete".to_string()) && paths.contains(&"/progress".to_string()));
    assert!(CalendarObject::from_value(json!({"@type":"Group"})).is_err());
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
    assert_eq!(v["recurrenceRule"], json!({"frequency":"weekly","count":12}));
    assert!(v.get("recurrenceRules").is_none() && v.get("timeZones").is_none() && v.get("excludedRecurrenceRules").is_none());
    assert_eq!(v["start"], "2026-11-03T10:00:00");
    let legacy = &v["freesynergy.net:legacy"];
    assert_eq!(legacy["recurrenceRules"], json!([{"frequency":"monthly"}]));
    assert!(legacy["timeZones"].is_object() && legacy["excludedRecurrenceRules"].is_array());
    assert_eq!(legacy["locations/l1/timeZone"], "Europe/Berlin");
    assert_eq!(legacy["participants/p1/language"], "de");
    assert_eq!(legacy["links/k/cid"], "abc");
    assert_eq!(legacy["startOriginal"], "2026-11-03T10:00:00.000");
    assert!(v["locations"]["l1"].get("timeZone").is_none());
    assert_eq!(v["participants"]["p1"]["sendTo"]["imip"], "mailto:b@x.org", "reserved props stay");
    assert!(up.notes.iter().any(|n| n.contains("further recurrence rule")));
    let obj = CalendarObject::from_value(v.clone()).unwrap();
    validate(&obj).unwrap();
    // idempotent
    assert_eq!(upgrade(up.value.clone()).value, up.value);
    // Z suffix and custom zone
    let up = upgrade(json!({"@type":"Task","uid":"t","due":"2026-11-12T17:00:00Z"}));
    assert_eq!((up.value["due"].as_str(), up.value["timeZone"].as_str()), (Some("2026-11-12T17:00:00"), Some("Etc/UTC")));
    let up = upgrade(json!({"@type":"Event","uid":"c","start":"2026-01-01T10:00:00","timeZone":"/Custom/Zone"}));
    assert!(up.value.get("timeZone").is_none() && up.value["freesynergy.net:legacy"]["timeZone"] == "/Custom/Zone");
    // Group
    let up = upgrade(json!({"@type":"Group","entries":[{"@type":"Event","uid":"a","recurrenceRules":[{"frequency":"daily"}]}]}));
    assert_eq!(up.value["entries"][0]["recurrenceRule"]["frequency"], "daily");
}

#[tokio::test]
async fn crud_envelope_index_and_sequence() {
    let r = repo().await;
    let mut e = Event::new("Jour fixe", "2026-11-03T10:00:00", "Europe/Berlin", "PT30M");
    e.recurrence_rule = Some(serde_json::from_value(json!({"frequency":"weekly","count":3})).unwrap());
    e.add_invitee("Ben", "Ben@Example.org");
    let o = r.create(e.clone()).await.unwrap();
    assert_eq!(o.envelope.object_type, "Event");
    assert_eq!(o.envelope.name, "Jour fixe");
    assert_eq!(o.envelope.context.as_deref(), Some("calendar"));
    assert_eq!(o.content_type, "application/jscalendar+json");
    assert_eq!(o.index["start_utc"], ["2026-11-03T09:00:00Z"]);
    assert_eq!(o.index["end_utc"], ["2026-11-17T09:30:00Z"]);
    assert_eq!(o.index["recurring"], ["true"]);
    assert_eq!(o.index["participant_email"], ["ben@example.org"]);
    assert!(o.envelope.iri.as_deref().unwrap().contains("/events/"));
    assert!(matches!(r.create(e.clone()).await, Err(Error::Conflict(_))));
    let id = o.envelope.id;
    let mut ev = r.get_event(id).await.unwrap().unwrap();
    assert_eq!(ev.sequence, None);
    ev.title = Some("Jour fixe (neu)".into());
    let o2 = r.update(id, ev.clone()).await.unwrap();
    assert_eq!(o2.envelope.name, "Jour fixe (neu)");
    assert_eq!(r.get_event(id).await.unwrap().unwrap().sequence, None, "title is not scheduling relevant");
    ev.start = Some("2026-11-03T11:00:00".into());
    r.update(id, ev.clone()).await.unwrap();
    let after = r.get_event(id).await.unwrap().unwrap();
    assert_eq!(after.sequence, Some(1));
    assert_eq!(r.get(id).await.unwrap().unwrap().index["sequence"], ["1"]);
    let mut other = ev.clone();
    other.uid = "other".into();
    assert!(matches!(r.update(id, other).await, Err(Error::Conflict(_))));
    r.delete(id).await.unwrap();
    assert!(r.get(id).await.unwrap().is_none());
    r.store().undo(ME, 1).await.unwrap();
    assert!(r.get(id).await.unwrap().is_some());
    // invalid input is refused
    let mut bad = Event::new("x", "2026-11-03T11:00:00", "Mars/Base", "PT1H");
    bad.uid = "bad".into();
    assert!(matches!(r.create(bad).await, Err(Error::Invalid(_))));
}

#[tokio::test]
async fn range_queries_expand_series() {
    let r = repo().await;
    let mut weekly = Event::new("Weekly", "2026-11-03T10:00:00", "Europe/Berlin", "PT1H");
    weekly.recurrence_rule = Some(serde_json::from_value(json!({"frequency":"weekly"})).unwrap());
    weekly.recurrence_overrides.insert("2026-11-10T10:00:00".into(), json!({"excluded": true}));
    let mut ended = Event::new("Ended", "2026-01-05T10:00:00", "Europe/Berlin", "PT1H");
    ended.recurrence_rule = Some(serde_json::from_value(json!({"frequency":"daily","count":3})).unwrap());
    let single = Event::new("Single", "2026-11-12T15:00:00", "Europe/Berlin", "PT2H");
    let past = Event::new("Past", "2026-10-01T15:00:00", "Europe/Berlin", "PT2H");
    for e in [weekly, ended, single, past] {
        r.create(e).await.unwrap();
    }
    let got = r.occurrences(t("2026-11-05T00:00:00Z"), t("2026-12-01T00:00:00Z")).await.unwrap();
    let names: Vec<_> = got.iter().map(|i| (i.occurrence.event.display_title(), i.occurrence.recurrence_id.clone())).collect();
    assert_eq!(
        names,
        [
            ("Single".to_string(), "2026-11-12T15:00:00".to_string()),
            ("Weekly".into(), "2026-11-17T10:00:00".into()),
            ("Weekly".into(), "2026-11-24T10:00:00".into()),
        ]
    );
    assert!(r.upcoming(t("2026-11-04T00:00:00Z"), 5).await.unwrap().is_empty(), "the 10th is excluded");
    let next = r.upcoming(t("2026-11-04T00:00:00Z"), 9).await.unwrap();
    assert_eq!(next.len(), 1);
    assert_eq!(next[0].occurrence.event.display_title(), "Single");
    // far future still finds the unbounded series
    let far = r.occurrences(t("2030-01-01T00:00:00Z"), t("2030-01-15T00:00:00Z")).await.unwrap();
    assert_eq!(far.len(), 2);
}

#[tokio::test]
async fn tasks_by_due_date() {
    let r = repo().await;
    let mut a = Task::new("Write the minutes");
    a.due = Some("2026-11-12T17:00:00".into());
    a.time_zone = Some("Europe/Berlin".into());
    a.set_vendor("freesynergy.net:taskStatus", json!("waiting-for-input"));
    let b = Task::new("Someday");
    let oa = r.create(a).await.unwrap();
    r.create(b).await.unwrap();
    assert_eq!(oa.envelope.object_type, "Task");
    assert_eq!(oa.index["due_utc"], ["2026-11-12T16:00:00Z"]);
    assert_eq!(oa.index["progress"], ["needs-action"]);
    let due = r.tasks_due(t("2026-11-01T00:00:00Z"), t("2026-12-01T00:00:00Z")).await.unwrap();
    assert_eq!(due.len(), 1);
    assert_eq!(r.list_tasks().await.unwrap().len(), 2);
    assert_eq!(r.list_tasks().await.unwrap()[0].envelope.name, "Write the minutes", "undated last");
    let mut task = r.get_task(oa.envelope.id).await.unwrap().unwrap();
    assert_eq!(task.vendor("freesynergy.net:taskStatus"), Some(&json!("waiting-for-input")));
    task.progress = Some("completed".into());
    task.percent_complete = Some(100);
    r.update(oa.envelope.id, task).await.unwrap();
    assert!(r.get_event(oa.envelope.id).await.unwrap().is_none());
    assert!(r.occurrences(t("2020-01-01T00:00:00Z"), t("2040-01-01T00:00:00Z")).await.unwrap().is_empty(), "tasks are not events");
}

#[tokio::test]
async fn invitations_are_log_activities() {
    let r = repo().await;
    let mut e = Event::new("Planning", "2026-11-24T10:00:00", "Europe/Berlin", "PT1H");
    e.add_invitee("Me", "me@example.org");
    let o = r.create(e).await.unwrap();
    let id = o.envelope.id;
    let o = r.invite(id, &[("Ben", "ben@example.org"), ("Me again", "ME@example.org")]).await.unwrap();
    let ev = Event::from_value(o.content.clone()).unwrap();
    assert_eq!(ev.participants.len(), 2, "existing address skipped");
    assert_eq!(o.index["participant_email"].len(), 2);

    r.respond(id, "ben@example.org", Response::TentativeAccept, Some("maybe")).await.unwrap();
    let ev = r.get_event(id).await.unwrap().unwrap();
    let key = ev.participant_by_email("BEN@example.org").unwrap().to_string();
    assert_eq!(ev.participants[&key].participation_status.as_deref(), Some("tentative"));
    assert_eq!(ev.participants[&key].participation_comment.as_deref(), Some("maybe"));
    r.respond(id, &key, Response::Accept, None).await.unwrap();
    assert_eq!(r.get_event(id).await.unwrap().unwrap().participants[&key].participation_status.as_deref(), Some("accepted"));
    r.respond(id, "me@example.org", Response::Reject, Some("clash")).await.unwrap();
    let ev = r.get_event(id).await.unwrap().unwrap();
    assert_eq!(ev.participants["p1"].participation_status.as_deref(), Some("declined"));
    assert!(matches!(r.respond(id, "nobody@example.org", Response::Accept, None).await, Err(Error::Conflict(_))));

    let log: Vec<_> = r.invitation_log(id).await.unwrap().into_iter().map(|e| (e.activity, e.email.unwrap_or_default())).collect();
    assert_eq!(
        log,
        [
            ("Invite".to_string(), "ben@example.org".to_string()),
            ("TentativeAccept".into(), "ben@example.org".into()),
            ("Accept".into(), "ben@example.org".into()),
            ("Reject".into(), "me@example.org".into()),
        ]
    );
    // mapped onto the store's log kinds
    let kinds: Vec<_> = r.store().activities(&ActivityFilter { object: Some(id), ..Default::default() }).await.unwrap().into_iter().map(|a| a.kind).collect();
    assert!(kinds.contains(&ActivityKind::Offer) && kinds.contains(&ActivityKind::Accept) && kinds.contains(&ActivityKind::Reject));
    // a cancelled event cannot be answered
    let mut ev = r.get_event(id).await.unwrap().unwrap();
    ev.status = Some("cancelled".into());
    r.update(id, ev).await.unwrap();
    assert!(matches!(r.respond(id, "ben@example.org", Response::Accept, None).await, Err(Error::Conflict(_))));
    assert!(r.get_event(id).await.unwrap().unwrap().sequence.unwrap() >= 1, "cancelling bumps the sequence");
}

#[tokio::test]
async fn import_demo_events() {
    let r = repo().await;
    let rep = r.import_jscalendar(&std::fs::read_to_string(DEMO).unwrap()).await.unwrap();
    assert_eq!((rep.created.len(), rep.skipped.len(), rep.failed.len()), (5, 0, 0), "{rep:?}");
    assert!(rep.notes.is_empty());
    let again = r.import_jscalendar(&std::fs::read_to_string(DEMO).unwrap()).await.unwrap();
    assert_eq!((again.created.len(), again.skipped.len()), (0, 5));
    let next = r.upcoming(t("2026-11-05T00:00:00Z"), 30).await.unwrap();
    assert_eq!(next.len(), 4, "the fifth draft is on 8 December");
    let next = r.upcoming(t("2026-11-05T00:00:00Z"), 40).await.unwrap();
    assert_eq!(next.len(), 5);
    let titles: Vec<_> = next.iter().map(|i| i.occurrence.event.display_title()).collect();
    assert!(titles[0].contains("NLnet") && titles[4].contains("Open Source Business Alliance"), "{titles:?}");
    assert_eq!(next[0].occurrence.start_utc, t("2026-11-24T09:00:00Z"));
    // only the first three fall into 20 days
    assert_eq!(r.upcoming(t("2026-11-05T00:00:00Z"), 22).await.unwrap().len(), 2);
    // accept: the organisation replies
    let first = next[0].object;
    r.respond(first, "webcontact@nlnet.nl", Response::Accept, None).await.unwrap();
    let ev = r.get_event(first).await.unwrap().unwrap();
    assert_eq!(ev.participants["p1"].participation_status.as_deref(), Some("accepted"));
    assert_eq!(ev.participants["owner"].participation_status.as_deref(), Some("accepted"));
}

#[tokio::test]
async fn import_rfc8984_data_upgrades() {
    let r = repo().await;
    let v1 = json!([{"@type":"Event","uid":"old-1","title":"Jour fixe","start":"2026-11-03T10:00:00","timeZone":"Europe/Berlin","duration":"PT30M",
        "recurrenceRules":[{"frequency":"weekly","count":2}]}]);
    let rep = r.import_jscalendar(&v1.to_string()).await.unwrap();
    assert_eq!(rep.created.len(), 1, "{rep:?}");
    assert!(!rep.notes.is_empty());
    let e = r.get_event(rep.created[0]).await.unwrap().unwrap();
    assert_eq!(e.version.as_deref(), Some("2.0"));
    assert_eq!(e.recurrence_rule.unwrap().count, Some(2));
}

const ICS: &str = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//x//y//EN\r\nBEGIN:VEVENT\r\nUID:ev-1\r\nDTSTAMP:20261101T000000Z\r\nDTSTART;TZID=Europe/Berlin:20261124T100000\r\nDTEND;TZID=Europe/Berlin:20261124T113000\r\nSUMMARY:Jour fixe\r\nDESCRIPTION:Hello\\, world\r\nLOCATION:Berlin\r\nRRULE:FREQ=WEEKLY;COUNT=4;BYDAY=TU\r\nEXDATE;TZID=Europe/Berlin:20261201T100000\r\nORGANIZER;CN=Me:mailto:me@example.org\r\nATTENDEE;CN=Ben;PARTSTAT=NEEDS-ACTION;RSVP=TRUE:mailto:ben@example.org\r\nSTATUS:CONFIRMED\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nUID:ev-2\r\nDTSTAMP:20261101T000000Z\r\nDTSTART:20261130T090000Z\r\nDURATION:PT2H\r\nSUMMARY:Second\r\nEND:VEVENT\r\nBEGIN:VTODO\r\nUID:todo-1\r\nDTSTAMP:20261101T000000Z\r\nDUE:20261112T170000Z\r\nSUMMARY:Minutes\r\nSTATUS:NEEDS-ACTION\r\nPERCENT-COMPLETE:10\r\nEND:VTODO\r\nEND:VCALENDAR\r\n";

#[test]
fn ics_is_split_per_uid_with_exact_text() {
    let parts = ical::split_ics(ICS);
    assert_eq!(parts.iter().map(|p| p.uid.as_str()).collect::<Vec<_>>(), ["ev-1", "ev-2", "todo-1"]);
    for p in &parts {
        assert!(p.text.starts_with("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//x//y//EN\r\nBEGIN:V"), "{}", p.text);
        assert!(p.text.ends_with("END:VCALENDAR\r\n"));
        assert_eq!(p.text.matches("BEGIN:VEVENT").count() + p.text.matches("BEGIN:VTODO").count(), 1);
        assert!(ICS.contains(p.text.lines().nth(4).unwrap()), "lines are copied unchanged");
    }
    // a single-UID file is kept whole
    let single = ical::split_ics(&parts[1].text);
    assert_eq!(single.len(), 1);
    assert_eq!(single[0].text, parts[1].text);
    assert!(ical::split_ics("nonsense").is_empty());
}

#[tokio::test]
async fn ics_import_keeps_original_and_converts() {
    let r = repo().await;
    let rep = r.import_ics(ICS).await.unwrap();
    assert_eq!((rep.created.len(), rep.skipped.len(), rep.failed.len()), (3, 0, 0), "{rep:?}");
    let ev = r.get(rep.created[0]).await.unwrap().unwrap();
    let orig = ev.original.as_ref().unwrap();
    assert_eq!(orig.media_type, "text/calendar");
    let text = String::from_utf8(orig.bytes.clone()).unwrap();
    assert!(text.contains("UID:ev-1") && !text.contains("UID:ev-2"));
    assert_eq!(ev.index["tzdb_version"], [fs_calendar_core::time::TZDB_VERSION]);
    assert_eq!(ev.envelope.name, "Jour fixe");
    let e = Event::from_value(ev.content.clone()).unwrap();
    assert_eq!(e.version.as_deref(), Some("2.0"));
    assert_eq!(e.start.as_deref(), Some("2026-11-24T10:00:00"));
    assert_eq!(e.time_zone.as_deref(), Some("Europe/Berlin"));
    assert_eq!(e.duration.as_deref(), Some("PT1H30M"));
    let rule = e.recurrence_rule.as_ref().unwrap();
    assert_eq!((rule.frequency.as_str(), rule.count), ("weekly", Some(4)));
    assert!(e.recurrence_overrides.contains_key("2026-12-01T10:00:00"), "EXDATE -> excluded override");
    assert_eq!(e.participants.len(), 2);
    // expansion: 24 Nov, (1 Dec excluded), 8 Dec, 15 Dec
    let occ = r.occurrences(t("2026-11-01T00:00:00Z"), t("2027-01-01T00:00:00Z")).await.unwrap();
    let starts: Vec<_> = occ.iter().filter(|i| i.object == rep.created[0]).map(|i| i.occurrence.recurrence_id.clone()).collect();
    assert_eq!(starts, ["2026-11-24T10:00:00", "2026-12-08T10:00:00", "2026-12-15T10:00:00"]);
    // the task
    let task = r.get(rep.created[2]).await.unwrap().unwrap();
    assert_eq!(task.envelope.object_type, "Task");
    assert_eq!(task.index["due_utc"], ["2026-11-12T17:00:00Z"]);
    // re-import skips
    let again = r.import_ics(ICS).await.unwrap();
    assert_eq!((again.created.len(), again.skipped.len()), (0, 3));
    // garbage
    let bad = r.import_ics("BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:x\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n").await.unwrap();
    assert_eq!(bad.created.len() + bad.failed.len(), 1, "no panic: imported or reported");
}

#[tokio::test]
async fn ics_export_roundtrip() {
    let r = repo().await;
    let rep = r.import_ics(ICS).await.unwrap();
    let out = r.export_ics(&rep.created).await.unwrap();
    for needle in ["BEGIN:VCALENDAR", "BEGIN:VEVENT", "BEGIN:VTODO", "UID:ev-1", "SUMMARY:Jour fixe", "RRULE:FREQ=WEEKLY;COUNT=4;BYDAY=TU", "DTSTART;TZID=Europe/Berlin:20261124T100000", "EXDATE;TZID=Europe/Berlin:20261201T100000", "mailto:ben@example.org", "DUE:20261112T170000Z"] {
        assert!(out.contains(needle), "{needle} in\n{out}");
    }
    // re-import the export into a fresh calendar: same structure
    let r2 = repo().await;
    let rep2 = r2.import_ics(&out).await.unwrap();
    assert_eq!(rep2.created.len(), 3, "{rep2:?}\n{out}");
    let a = r.get_event(rep.created[0]).await.unwrap().unwrap();
    let b = r2.list_events().await.unwrap().into_iter().find_map(|o| Event::from_value(o.content).ok().filter(|e| e.uid == "ev-1")).unwrap();
    assert_eq!((a.start, a.time_zone, a.duration, a.recurrence_rule), (b.start, b.time_zone, b.duration, b.recurrence_rule));
    // a native JSCalendar event exports too
    let mut e = Event::new("Native", "2026-11-24T10:00:00", "Europe/Berlin", "PT1H");
    e.add_invitee("Ben", "ben@example.org");
    let id = r.create(e).await.unwrap().envelope.id;
    let out = r.export_ics(&[id]).await.unwrap();
    assert!(out.contains("SUMMARY:Native") && out.contains("DTSTART;TZID=Europe/Berlin:20261124T100000"), "{out}");
}

#[tokio::test]
async fn demo_events_export_as_ics() {
    let r = repo().await;
    let rep = r.import_jscalendar(&std::fs::read_to_string(DEMO).unwrap()).await.unwrap();
    let out = r.export_ics(&rep.created).await.unwrap();
    assert_eq!(out.matches("BEGIN:VEVENT").count(), 5);
    assert!(out.contains("mailto:webcontact@nlnet.nl") && out.contains("DTSTART;TZID=Europe/Berlin:20261124T100000"));
}
