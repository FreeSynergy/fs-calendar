//! Reading JSON with limits, stopping at the first value past them.
//!
//! Pattern: Visitor — a serde `Visitor` that builds a `serde_json::Value` node by node while it counts nodes, levels and
//! estimated memory (`weight`), and gives up at once when a limit is passed, so the rest of a text
//! of millions of small values is never built. `serde_json::from_str` builds all of it first.

use std::cell::Cell;
use std::fmt;

use serde::de::{DeserializeSeed, Deserializer, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Number, Value};

use super::counter::{Counter, MARK};
use super::weight::{ARRAY_BYTES, NODE_BYTES, OBJECT_BYTES, STRING_BYTES};
use crate::error::{Error, Result};
use crate::limits::{MAX_EVENT_DEPTH, MAX_EVENT_MEMORY_BYTES, MAX_EVENT_NODES};

/// What a text may hold.
#[derive(Clone, Copy, Debug)]
pub struct Bounds {
    /// Most values.
    pub nodes: usize,
    /// Most levels of arrays and objects inside each other.
    pub depth: usize,
    /// Most estimated bytes of memory.
    pub memory: usize,
}

impl Bounds {
    /// The limits of one event.
    pub const EVENT: Self = Self {
        nodes: MAX_EVENT_NODES,
        depth: MAX_EVENT_DEPTH,
        memory: MAX_EVENT_MEMORY_BYTES,
    };
}

impl Bounds {
    /// The limits of a document of several events: what a file of events may hold in all.
    pub const DOCUMENT: Self = Self {
        nodes: 20 * MAX_EVENT_NODES,
        depth: MAX_EVENT_DEPTH + 2,
        memory: 4 * MAX_EVENT_MEMORY_BYTES,
    };
}

struct Seed<'a> {
    counter: &'a Counter,
    depth: usize,
}

impl<'de> DeserializeSeed<'de> for Seed<'_> {
    type Value = Value;

    fn deserialize<D: Deserializer<'de>>(self, d: D) -> std::result::Result<Value, D::Error> {
        d.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for Seed<'_> {
    type Value = Value;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("any JSON value")
    }

    fn visit_unit<E: serde::de::Error>(self) -> std::result::Result<Value, E> {
        self.counter.charge::<E>(NODE_BYTES)?;
        Ok(Value::Null)
    }

    fn visit_bool<E: serde::de::Error>(self, v: bool) -> std::result::Result<Value, E> {
        self.counter.charge::<E>(NODE_BYTES)?;
        Ok(Value::Bool(v))
    }

    fn visit_i64<E: serde::de::Error>(self, v: i64) -> std::result::Result<Value, E> {
        self.counter.charge::<E>(NODE_BYTES)?;
        Ok(Value::from(v))
    }

    fn visit_u64<E: serde::de::Error>(self, v: u64) -> std::result::Result<Value, E> {
        self.counter.charge::<E>(NODE_BYTES)?;
        Ok(Value::from(v))
    }

    fn visit_f64<E: serde::de::Error>(self, v: f64) -> std::result::Result<Value, E> {
        self.counter.charge::<E>(NODE_BYTES)?;
        Ok(Number::from_f64(v).map_or(Value::Null, Value::Number))
    }

    fn visit_str<E: serde::de::Error>(self, v: &str) -> std::result::Result<Value, E> {
        self.counter
            .charge::<E>(NODE_BYTES + v.len() + STRING_BYTES)?;
        Ok(Value::String(v.to_string()))
    }

    fn visit_string<E: serde::de::Error>(self, v: String) -> std::result::Result<Value, E> {
        self.counter
            .charge::<E>(NODE_BYTES + v.len() + STRING_BYTES)?;
        Ok(Value::String(v))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> std::result::Result<Value, A::Error> {
        if self.depth >= self.counter.bounds.depth {
            return Err(serde::de::Error::custom(format!(
                "{MARK}more than {} levels",
                self.counter.bounds.depth
            )));
        }
        self.counter.charge::<A::Error>(NODE_BYTES + ARRAY_BYTES)?;
        let mut items = Vec::new();
        while let Some(item) = seq.next_element_seed(Seed {
            counter: self.counter,
            depth: self.depth + 1,
        })? {
            // The element and the room its array may hold for one more while it grows.
            self.counter.extra::<A::Error>(2 * NODE_BYTES)?;
            items.push(item);
        }
        items.shrink_to_fit();
        Ok(Value::Array(items))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> std::result::Result<Value, A::Error> {
        if self.depth >= self.counter.bounds.depth {
            return Err(serde::de::Error::custom(format!(
                "{MARK}more than {} levels",
                self.counter.bounds.depth
            )));
        }
        self.counter.charge::<A::Error>(NODE_BYTES + OBJECT_BYTES)?;
        let mut members = Map::new();
        while let Some(key) = map.next_key::<String>()? {
            self.counter.entry::<A::Error>(&key, members.len() + 1)?;
            let value = map.next_value_seed(Seed {
                counter: self.counter,
                depth: self.depth + 1,
            })?;
            members.insert(key, value);
        }
        Ok(Value::Object(members))
    }
}

/// Read `text` as JSON, refusing it at the first value past `bounds` ([`Error::TooHeavy`]) and when it
/// is no JSON ([`Error::Json`]).
pub fn parse_bounded(text: &str, bounds: Bounds) -> Result<Value> {
    let counter = Counter {
        bounds,
        nodes: Cell::new(0),
        memory: Cell::new(0),
    };
    let mut de = serde_json::Deserializer::from_str(text);
    let found = Seed {
        counter: &counter,
        depth: 0,
    }
    .deserialize(&mut de)
    .and_then(|value| de.end().map(|()| value));
    found.map_err(|e| match e.to_string().split_once(MARK) {
        Some((_, why)) => Error::TooHeavy(why.split(" at line").next().unwrap_or(why).to_string()),
        None => Error::Json(e),
    })
}
