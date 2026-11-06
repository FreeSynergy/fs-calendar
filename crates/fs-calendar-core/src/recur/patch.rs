//! JSCalendar `PatchObject` application (RFC 8984 section 1.4.9).
//!
//! Pattern: Command (applied patch) — a patch is a map of path -> new value (`null` removes);
//! applying it to a base value yields the overridden instance.

use serde_json::Value;

/// Apply a JSCalendar `PatchObject` (`{"title": "x", "participants/p1/name": "y", "locations/l1": null}`).
pub fn apply_patch(mut base: Value, patch: &serde_json::Map<String, Value>) -> Value {
    for (path, val) in patch {
        let parts: Vec<&str> = path.split('/').collect();
        set_path(&mut base, &parts, val.clone());
    }
    base
}

fn set_path(target: &mut Value, parts: &[&str], val: Value) {
    let Some((head, rest)) = parts.split_first() else {
        return;
    };
    let head = head.replace("~1", "/").replace("~0", "~");
    if !target.is_object() {
        *target = Value::Object(serde_json::Map::default());
    }
    let obj = target.as_object_mut().unwrap();
    if rest.is_empty() {
        if val.is_null() {
            obj.remove(&head);
        } else {
            obj.insert(head, val);
        }
    } else {
        set_path(obj.entry(head).or_insert(Value::Null), rest, val);
    }
}
