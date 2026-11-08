//! How an event is printed.
//!
//! Pattern: none (pure functions of a JSCalendar text).

use serde_json::Value;

/// Control characters in a value (a newline, a terminal escape) are shown as `?`.
fn clean(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_control() { '?' } else { c })
        .collect()
}

/// `id  start  title`, columns split by two spaces; a missing member is `-`.
pub(crate) fn line(id: &str, jscalendar: &str) -> String {
    let v: Value = serde_json::from_str(jscalendar).unwrap_or(Value::Null);
    let text = |k: &str| {
        v.get(k)
            .and_then(Value::as_str)
            .map_or_else(|| "-".to_string(), clean)
    };
    format!("{id}  {}  {}", text("start"), text("title"))
}

/// The JSCalendar text pretty-printed; the text as it is when it is no JSON.
pub(crate) fn pretty(jscalendar: &str) -> String {
    serde_json::from_str::<Value>(jscalendar)
        .and_then(|v| serde_json::to_string_pretty(&v))
        .unwrap_or_else(|_| jscalendar.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_has_three_columns_and_dashes_for_gaps() {
        let l = line("1", r#"{"title":"Talk","start":"2026-12-01T10:00:00"}"#);
        assert_eq!(l, "1  2026-12-01T10:00:00  Talk");
        assert_eq!(line("2", "not json"), "2  -  -");
    }
}
