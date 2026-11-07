//! The sensitive marking of an event (ADR 0029).
//!
//! Pattern: Iterator — the marking `freesynergy.net:sensitive` is allowed on the event and on every
//! object inside it, so the serialised event is walked by an iterator over its markings. What it names becomes the
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
        let value = self.to_value();
        for marking in Markings::of(&value) {
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
        }
        if unclear {
            named.push(UNCLEAR_SENSITIVE.to_string());
        }
        named.sort();
        named.dedup();
        named
    }
}

/// Every value of the marking property in a JSON value, at any depth.
struct Markings<'a> {
    stack: Vec<&'a Value>,
    found: Vec<&'a Value>,
}

impl<'a> Markings<'a> {
    fn of(value: &'a Value) -> Self {
        Self {
            stack: vec![value],
            found: Vec::new(),
        }
    }
}

impl<'a> Iterator for Markings<'a> {
    type Item = &'a Value;

    fn next(&mut self) -> Option<&'a Value> {
        while self.found.is_empty() {
            match self.stack.pop()? {
                Value::Object(members) => {
                    for (key, member) in members {
                        if key == SENSITIVE_KEY {
                            self.found.push(member);
                        } else {
                            self.stack.push(member);
                        }
                    }
                }
                Value::Array(items) => self.stack.extend(items),
                _ => {}
            }
        }
        self.found.pop()
    }
}
