//! The words of the program.
//!
//! Pattern: Value objects — all English text of this frontend in one place; translation (ADR 0053)
//! will replace this file's content, not its users.

use fs_components_workbenches::calendar::CalendarLabels;

/// The title of the window.
pub const TITLE: &str = "FreeSynergy Calendar";

/// The words of the workbench.
pub fn labels() -> CalendarLabels {
    CalendarLabels::default()
}

/// Headings and labels of what is shown about an event.
pub(crate) mod field {
    pub const SENSITIVE: &str = "Sensitive";
    pub const TITLE: &str = "Title";
    pub const START: &str = "Start";
    pub const END: &str = "End";
    pub const ALL_DAY: &str = "All day";
    pub const YES: &str = "Yes";
    pub const ZONE: &str = "Time zone";
    pub const FLOATING: &str = "none (the clock wherever you are)";
    pub const LOCATION: &str = "Location";
    pub const DESCRIPTION: &str = "Description";
    pub const STATUS: &str = "Status";
    pub const WHEN: &str = "When";
    pub const HERE: &str = "In the zone shown";
    pub const WHAT: &str = "Details";
    pub const REPEATS: &str = "Repeats";
    pub const RULE: &str = "Rule";
    pub const SERIES_TITLE: &str = "Series title";
    pub const THIS_ONE: &str = "This occurrence";
    pub const PEOPLE: &str = "Participants";
    pub const OTHER: &str = "Kept on the event";
    pub const OTHER_LABEL: &str = "Not shown here";
    pub const TASK: &str = "Task";
    pub const DUE: &str = "Due";
    pub const PROGRESS: &str = "Progress";
}

/// Sentences about what an edit can do.
pub(crate) mod say {
    pub const SERIES_SHEET: &str = "This event repeats. Edit changes the title, location and description of the series; an occurrence with values of its own keeps them. One occurrence alone, the times and the rule cannot be changed here.";
    pub const OWN_VALUES: &str = "This occurrence has values of its own, so an edit of the series does not change what it shows.";
    pub const SERIES_FORM: &str = "You are editing the series: all its occurrences but what one of them has of its own. Its times cannot be changed here, because that could shift or lose occurrences.";
    pub const SERIES_REFUSED: &str = "this event repeats, and changing its times here could shift or lose occurrences. Nothing was changed";
    pub const SERIES_DELETE: &str = "It repeats: all its occurrences go.";
    pub const MORE_PLACES: &str =
        "This event has several locations. The form shows the first; the others stay as they are.";
    pub const TASK_NOTE: &str =
        "This is a task of the tasks program. It is shown here and changed there.";
    pub const NEEDS_TITLE: &str = "enter a title";
    pub const END_BEFORE_START: &str = "the end is before the start";
    pub const IN_GAP: &str = "{time} does not exist in {zone}: the clocks jump forward then. Enter a time before or after it";
    pub const AFTER_FOLD: &str = "(after the clocks went back)";
    pub const DAMAGED: &str = "{title}: cannot be shown ({why}). Open it to read or delete it";
    pub const TASKS_LOST: &str = "Tasks are not shown while an event is damaged";
    pub const TOO_MANY: &str =
        "{title}: too many occurrences for these days, only the first {count} are shown";
    pub const IMPORT_PANIC: &str = "something in this file cannot be handled, a length far too long perhaps; what came before it is in";
}
