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

/// A sentence with its named placeholders (`{name}`) filled from `values`; the sentence stays whole,
/// so a language may put the parts in another order.
pub(crate) fn fill(sentence: &str, values: &[(&str, &str)]) -> String {
    values
        .iter()
        .fold(sentence.to_string(), |text, (name, value)| {
            text.replace(&format!("{{{name}}}"), value)
        })
}

/// What the command line says to a person.
pub(crate) mod cli {
    pub const NO_SOCKET: &str = "no --socket given: the app talks to a running calendar server; start one with `fs-calendar-server --socket <path>` and pass the same path here";
    pub const STORE_GONE: &str = "--store is gone: the store belongs to the server, start it with `fs-calendar-server --data <dir>` and give the app its --socket";
    pub const NEEDS_VALUE: &str = "{option} needs a value";
    pub const UNKNOWN_OPTION: &str = "unknown option {option}";
    pub const SENSITIVE_NEEDS_EXPORT: &str = "--export-sensitive needs --export";
    pub const BAD_DATE: &str =
        "--date wants YYYY-MM-DD in the years {first} to {last}, not {given}";
    pub const BAD_VIEW: &str = "--view wants month, week, day or agenda, not {given}";
    pub const BAD_WEEK_START: &str = "--week-start wants monday, saturday or sunday, not {given}";
    pub const CANNOT_REACH: &str = "cannot reach the calendar server at {socket}: {error}";
    pub const TASKS_CANNOT_REACH: &str = "cannot reach the tasks server at {socket}: {error}";
    pub const NO_TASKS_SOCKET: &str = "No --tasks-socket given, so no tasks are shown";
    pub const NOT_EXPORTED: &str = "{count} could not be exported";
    pub const WROTE: &str = "wrote {file}";
}

/// The app has no `--socket`.
pub fn no_socket() -> String {
    cli::NO_SOCKET.to_string()
}

/// The app could not connect to the calendar server.
pub fn cannot_reach(socket: &str, error: &str) -> String {
    fill(cli::CANNOT_REACH, &[("socket", socket), ("error", error)])
}

/// The app could not connect to the tasks server.
pub fn tasks_cannot_reach(socket: &str, error: &str) -> String {
    fill(
        cli::TASKS_CANNOT_REACH,
        &[("socket", socket), ("error", error)],
    )
}

/// The calendar runs without tasks.
pub fn no_tasks_socket() -> String {
    cli::NO_TASKS_SOCKET.to_string()
}

/// Some events could not be exported.
pub fn not_exported(count: usize) -> String {
    fill(cli::NOT_EXPORTED, &[("count", &count.to_string())])
}

/// A picture was written.
pub fn wrote(file: &str) -> String {
    fill(cli::WROTE, &[("file", file)])
}

/// The help text.
pub const HELP: &str = "fs-calendar-app: the calendar program

  --socket <path>    Unix socket of a running `fs-calendar-server --socket <path>` (required)
  --tasks-socket <path>
                     Unix socket of a running `fs-tasks-server`, to show the tasks that are due
                     (optional; without it no task is shown)
  --import <file>    import an iCalendar file first; events already known are skipped, never replaced
  --export <file>    write all events to a new iCalendar file and stop; an existing file is not
                     replaced, events marked sensitive are left out and named
  --export-sensitive <uid>
                     release this event marked sensitive for --export; repeat for each event
  --snapshot <png>   draw a picture instead of opening a window
  --size <WxH>       window or picture size (default 1300x800)
  --date <YYYY-MM-DD>
                     fix today to this date, at noon, so that pictures are the same every time
  --zone <Area/City> the time zone shown (default: the system's)
  --view <month|week|day|agenda>
                     the view to start with (default month)
  --week-start <monday|saturday|sunday>
                     the first day of a week (default monday)";

/// The menus of the window.
pub(crate) mod menu {
    pub const EVENT: &str = "Event";
    pub const NEW: &str = "New";
    pub const VIEW: &str = "View";
    pub const MONTH: &str = "Month";
    pub const WEEK: &str = "Week";
    pub const DAY: &str = "Day";
    pub const AGENDA: &str = "Agenda";
    pub const GO: &str = "Go";
    pub const TODAY: &str = "Today";
    pub const PREVIOUS: &str = "Previous";
    pub const NEXT: &str = "Next";
}

/// Headings and labels of what is shown about an event.
pub(crate) mod field {
    pub const SENSITIVE: &str = "Sensitive";
    pub const TITLE: &str = "Title";
    pub const START: &str = "Start";
    pub const END: &str = "End";
    pub const ALL_DAY: &str = "All day";
    pub const YES: &str = "Yes";
    pub const NO: &str = "No";
    pub const NONE: &str = "none";
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
    pub const MARKED_ONE: &str = "An event";
    pub const MARKED_LOST: &str = "an event marked sensitive";
    pub const CHANGED: &str = "Someone else changed this event since you started.";
    pub const EVENT_GONE: &str = "This event no longer exists.";
    pub const TASKS_FAILED: &str = "Tasks cannot be shown: {why}";
    pub const CAUGHT: &str =
        "a length far too long, or something else the time library cannot hold";
    pub const TIMES_CLASH: &str = "the times of this event were changed elsewhere ({changes}), so yours were not written and nothing was changed. Put your times back as they were to save the rest, or Cancel and start again from the current times";
    pub const END_AFTER_FOLD: &str =
        "The clocks go back during this event, so its end reads earlier than its start.";
    pub const TOO_MANY: &str =
        "{title}: too many occurrences for these days, only the first {count} are shown";
    pub const IMPORT_PANIC: &str =
        "not imported: a length far too long, or something else the time library cannot hold";
}
