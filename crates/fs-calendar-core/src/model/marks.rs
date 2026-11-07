//! The sensitive marking of an event (ADR 0029).
//!
//! Pattern: Visitor (recursive walk) — the marking `freesynergy.net:sensitive` is allowed on the event
//! and on every object inside it, so the serialised event is walked. What it names becomes the
//! categories of the envelope; a marking that names none fails closed as [`UNCLEAR_SENSITIVE`].

use serde_json::Value;

use super::Event;

/// The vendor property that marks data as sensitive (the key `fs-people` uses).
pub const SENSITIVE_KEY: &str = "freesynergy.net:sensitive";
/// In the envelope of an event marked without a category we can read: not a DPV category, only what
/// makes the store and every reader treat the event as sensitive until it is clarified.
pub const UNCLEAR_SENSITIVE: &str = "freesynergy.net:unclear";

impl Event {
    /// All DPV categories marked anywhere in the event, sorted, unique; [`UNCLEAR_SENSITIVE`] if
    /// something is marked without a readable category.
    pub fn sensitive_categories(&self) -> Vec<String> {
        let (mut named, mut unclear) = (Vec::new(), false);
        visit(&self.to_value(), &mut |marking| {
            let before = named.len();
            match marking {
                Value::Array(list) => named.extend(
                    list.iter()
                        .filter_map(Value::as_str)
                        .filter(|t| !t.trim().is_empty())
                        .map(str::to_string),
                ),
                Value::String(text) if !text.trim().is_empty() => named.push(text.clone()),
                _ => {}
            }
            unclear |= named.len() == before;
        });
        if unclear {
            named.push(UNCLEAR_SENSITIVE.to_string());
        }
        named.sort();
        named.dedup();
        named
    }
}

fn visit(value: &Value, found: &mut dyn FnMut(&Value)) {
    match value {
        Value::Object(members) => {
            for (key, member) in members {
                if key == SENSITIVE_KEY {
                    found(member);
                } else {
                    visit(member, found);
                }
            }
        }
        Value::Array(items) => items.iter().for_each(|i| visit(i, found)),
        _ => {}
    }
}
