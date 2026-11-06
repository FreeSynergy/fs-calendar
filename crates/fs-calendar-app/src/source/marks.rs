//! How to tell an event is marked sensitive (ADR 0029).
//!
//! Pattern: Specification — fails closed: a category on the envelope, the vendor property anywhere in
//! the content, in a location or an override too, or a JSCalendar `privacy` other than `public` counts
//! as marked. Provisional: `freesynergy.net:sensitive` is the key `fs-people` uses; the calendar core
//! has no marking of its own yet.

use fs_store::StoredObject;
use serde_json::Value;

/// The vendor property that marks data as sensitive.
pub(super) const SENSITIVE_KEY: &str = "freesynergy.net:sensitive";

/// Is this content marked in any form?
pub(super) fn content_is_marked(content: &Value) -> bool {
    let private = content["privacy"]
        .as_str()
        .is_some_and(|privacy| privacy != "public");
    private || content.to_string().contains(SENSITIVE_KEY)
}

/// Is this stored event marked sensitive in any form?
pub fn is_marked(object: &StoredObject) -> bool {
    !object.envelope.sensitive.is_empty() || content_is_marked(&object.content)
}
