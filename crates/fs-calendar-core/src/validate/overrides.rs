//! The overrides of a series must make valid instances.
//!
//! Pattern: none (a pure check; coding.md section 2 has no Specification) — each patch is applied to the master the way the expansion does, and the
//! instance it gives is held to the rules of an event. An override that would not read is refused when
//! it is written, so it never costs a calendar its answers later.

use fs_jscalendar::validate::Ctx;
use serde_json::Value;

use super::check::issues;
use crate::model::Event;
use crate::recur::{apply_patch, master_value};

/// Report every override of `e` whose instance is not a valid event.
pub(super) fn check_overrides(c: &mut Ctx, e: &Event) {
    let master = master_value(e);
    for (key, patch) in &e.recurrence_overrides {
        let Some(members) = patch.as_object() else {
            continue; // the shared rules say what a patch must be
        };
        if members.get("excluded") == Some(&Value::Bool(true)) {
            continue;
        }
        let mut base = master.clone();
        if let Some(members) = base.as_object_mut() {
            members.insert("recurrenceId".into(), Value::String(key.clone()));
            members.insert("start".into(), Value::String(key.clone()));
        }
        let mut members = members.clone();
        members.remove("excluded");
        let at = format!("/recurrenceOverrides/{}", fs_jscalendar::quoted(key));
        match Event::from_value(apply_patch(base, &members)) {
            Err(err) => c.bad(&at, format!("the patch does not give an event: {err}")),
            Ok(instance) => {
                for issue in issues(&instance) {
                    c.bad(&format!("{at}{}", issue.path), issue.message);
                }
            }
        }
    }
}
