//! The words of the window (ADR 0057): English, from keys.
//!
//! Pattern: Strategy behind `Say` — a table for the keys of the shell, the admin frontend and the grid, and a rule for
//! the keys of the Event description: `event.time-zone` is "Time zone", `event.import.format` is "Format". A key
//! nobody wrote shows as the key, so a missing word is seen and not hidden. The catalog of ADR 0057 replaces this file.

use fs_components_fields::Say;

const TABLE: &[(&str, &str)] = &[
    ("menu.program", "Program"),
    ("menu.quit", "Quit"),
    ("menu.view", "View"),
    ("menu.help", "Help"),
    ("menu.about", "About"),
    ("event", "Event"),
    ("type.Event", "Events"),
    ("screen.calendar", "Calendar"),
    ("action.add", "Add"),
    ("action.back", "Back"),
    ("action.cancel", "Cancel"),
    ("action.close", "Close"),
    ("action.remove", "Remove"),
    ("action.save", "Save"),
    ("confirm.sure", "Are you sure?"),
    ("list.empty", "Nothing here yet."),
    (
        "list.stale",
        "These entries may be old: the program could not be reached.",
    ),
    ("list.separator", ", "),
    ("value.none", "none"),
    ("detail.not-loaded", "This entry is not loaded."),
    ("about.unoffered", "Not offered here"),
    ("message.rejected", "The program did not accept the input."),
    ("message.not-found", "This entry does not exist any more."),
    ("message.unavailable", "The program cannot do this now."),
    (
        "message.unsaved",
        "There is text in a form that was not saved.",
    ),
    ("message.done", "Done."),
    ("calendar.previous", "Previous"),
    ("calendar.today", "Today"),
    ("calendar.next", "Next"),
    ("calendar.mode.month", "Month"),
    ("calendar.mode.week", "Week"),
    ("calendar.mode.day", "Day"),
    ("find.term", "Search"),
    ("find.items", "Events"),
];

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];
const DAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

/// The window's words.
#[derive(Clone, Copy, Debug, Default)]
pub struct Words;

fn readable(part: &str) -> String {
    let mut text = part.replace('-', " ");
    if let Some(first) = text.get_mut(0..1) {
        first.make_ascii_uppercase();
    }
    text
}

impl Say for Words {
    fn say(&self, key: &str) -> String {
        if let Some((_, text)) = TABLE.iter().find(|(k, _)| *k == key) {
            return (*text).to_owned();
        }
        if let Some(n) = key
            .strip_prefix("calendar.month.")
            .and_then(|n| n.parse::<usize>().ok())
        {
            if let Some(name) = n.checked_sub(1).and_then(|i| MONTHS.get(i)) {
                return (*name).to_owned();
            }
        }
        if let Some(n) = key
            .strip_prefix("calendar.weekday.")
            .and_then(|n| n.parse::<usize>().ok())
        {
            if let Some(name) = DAYS.get(n) {
                return (*name).to_owned();
            }
        }
        if let Some(rest) = key.strip_prefix("hint.event.") {
            return match rest.rsplit('.').next().unwrap_or("") {
                "start" => "YYYY-MM-DDTHH:MM:SS (local time)".to_owned(),
                "duration" => "for example PT1H or P1D".to_owned(),
                "time-zone" | "end-time-zone" => "for example Europe/Berlin".to_owned(),
                "locations" => "geo:48.2,16.37".to_owned(),
                "links" => "https://".to_owned(),
                "participants" => "mailto:name@example.org".to_owned(),
                "keywords" => "words separated by commas".to_owned(),
                _ => String::new(),
            };
        }
        match key.strip_prefix("event.") {
            Some(rest) => readable(rest.rsplit('.').next().unwrap_or(rest)),
            None => key.to_owned(),
        }
    }
}
