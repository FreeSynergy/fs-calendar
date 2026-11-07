//! What is marked in an export: left out and named, or released, marked, and for its owner only.
//!
//! Integration tests through the real server and client; files in a scratch folder.

mod common;

use common::{Scratch, World, rich};
use fs_calendar_app::{export_all, import_file};
use serde_json::json;

fn export(
    w: &World,
    path: &std::path::Path,
    released: &[String],
) -> Result<fs_calendar_app::ExportReport, String> {
    export_all(w.client.as_ref(), w.runtime.handle(), path, released)
}

fn sensitive_world() -> (World, String) {
    let w = World::berlin();
    w.add_json(rich());
    let mut secret = rich();
    let uid = "urn:uuid:01a1aaf0-08a0-756c-85b2-f092789b0a07".to_string();
    secret["uid"] = json!(uid);
    secret["title"] = json!("Clinic");
    secret["freesynergy.net:sensitive"] = json!(["pd:HealthRecord"]);
    w.add_json(secret);
    (w, uid)
}

#[test]
fn a_sensitive_event_stays_out_of_an_export_and_is_named() {
    let dir = Scratch::new();
    let (w, uid) = sensitive_world();
    let out = dir.file("out.ics");
    let report = export(&w, &out, &[]).unwrap();
    assert_eq!(report.written, 1);
    assert!(report.left_out[0].contains("Clinic") && report.left_out[0].contains(&uid));
    assert!(report.message(&out).contains("--export-sensitive"));
    let text = std::fs::read_to_string(&out).unwrap();
    assert!(!text.contains("Clinic"), "{text}");
    assert!(text.contains("Kick-off"));
}

#[test]
fn naming_one_releases_that_one_and_naming_nothing_writes_nothing() {
    let dir = Scratch::new();
    let (w, uid) = sensitive_world();
    let out = dir.file("out.ics");
    let wrong = export(&w, &out, &["urn:uuid:nobody".to_string()]);
    assert!(wrong.unwrap_err().contains("names no event"));
    assert!(!out.exists(), "nothing was written");
    let report = export(&w, &out, &[uid]).unwrap();
    assert_eq!(report.written, 2);
    assert!(report.left_out.is_empty());
    assert!(report.released[0].contains("Clinic"));
    assert!(std::fs::read_to_string(&out).unwrap().contains("Clinic"));
}

#[test]
fn a_released_event_marked_only_on_its_envelope_comes_back_marked_and_the_file_is_private() {
    let (w, dir) = (World::berlin(), Scratch::new());
    let mut secret = rich();
    secret["title"] = json!("Clinic");
    let key = w.add_json(secret);
    let mut object = w.stored(&key);
    object.envelope.sensitive = vec!["pd:HealthRecord".into()];
    let uid = object.content["uid"].as_str().unwrap().to_string();
    w.runtime.block_on(w.store.put(object, common::ME)).unwrap();
    let out = dir.file("released.ics");
    let report = export(&w, &out, std::slice::from_ref(&uid)).unwrap();
    assert!(
        report.message(&out).contains("marked private"),
        "{}",
        report.message(&out)
    );
    let text = std::fs::read_to_string(&out).unwrap();
    assert!(text.contains("CLASS:PRIVATE"), "{text}");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&out).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "a sensitive release is for its owner");
    }
    let other = World::berlin();
    let back = import_file(other.client.as_ref(), other.runtime.handle(), &out).unwrap();
    assert_eq!(back.created, 1, "{}", back.message());
    assert_eq!(
        other.item("2026-11-24", "Clinic").marks,
        ["Sensitive"],
        "it comes back marked"
    );
    let stored = other.stored_uid(&uid).to_string();
    eprintln!("category back: {}", stored.contains("pd:HealthRecord"));
}

#[test]
fn an_export_without_a_release_is_not_made_private() {
    let (w, dir) = (World::berlin(), Scratch::new());
    w.add_json(rich());
    let out = dir.file("plain.ics");
    export(&w, &out, &[]).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&out).unwrap().permissions().mode() & 0o777;
        assert_ne!(mode, 0o600);
    }
}
