//! The command line.
//!
//! Pattern: none — pure functions: the options in, an `Args` or a sentence of `words.rs` out.

use std::path::PathBuf;

use fs_components_workbenches::calendar::{Day, FIRST_YEAR, LAST_YEAR, ViewRange};

use crate::words::{cli, fill};

pub use crate::words::HELP;

/// What was asked for on the command line.
#[derive(Debug, PartialEq, Eq)]
pub struct Args {
    /// Unix socket of a running `fs-calendar-server`; the app holds no store of its own.
    pub socket: Option<PathBuf>,
    /// Unix socket of a running `fs-tasks-server`, to show the tasks that are due; optional.
    pub tasks_socket: Option<PathBuf>,
    /// An iCalendar file to import before anything else.
    pub import: Option<PathBuf>,
    /// Write all events to this iCalendar file; no window opens then.
    pub export: Option<PathBuf>,
    /// The `uid`s of events marked sensitive that the export may include, each named by the person.
    pub export_sensitive: Vec<String>,
    /// Draw a picture to this file instead of opening a window.
    pub snapshot: Option<PathBuf>,
    /// Size of the window or picture.
    pub size: (u32, u32),
    /// Fix today to this date, so that pictures and tests are the same every time.
    pub date: Option<Day>,
    /// The time zone shown; `None` = the system's.
    pub zone: Option<String>,
    /// The view to start with.
    pub view: Option<ViewRange>,
    /// The first day of a week: 0 is Monday, 6 is Sunday.
    pub week_start: u8,
    /// `--help` was given.
    pub help: bool,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            socket: None,
            tasks_socket: None,
            import: None,
            export: None,
            export_sensitive: Vec::new(),
            snapshot: None,
            size: (1300, 800),
            date: None,
            zone: None,
            view: None,
            week_start: 0,
            help: false,
        }
    }
}

/// Parse `args` (without the program name).
///
/// # Errors
/// A message for the person: an unknown option, a missing value or a bad size, date or view.
pub fn parse(args: &[String]) -> Result<Args, String> {
    let mut out = Args::default();
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        let mut value = |name: &str| {
            it.next()
                .cloned()
                .ok_or_else(|| fill(cli::NEEDS_VALUE, &[("option", name)]))
        };
        match arg.as_str() {
            "--socket" => out.socket = Some(PathBuf::from(value("--socket")?)),
            "--tasks-socket" => out.tasks_socket = Some(PathBuf::from(value("--tasks-socket")?)),
            "--store" => return Err(cli::STORE_GONE.to_string()),
            "--import" => out.import = Some(PathBuf::from(value("--import")?)),
            "--export" => out.export = Some(PathBuf::from(value("--export")?)),
            "--export-sensitive" => out.export_sensitive.push(value("--export-sensitive")?),
            "--snapshot" => out.snapshot = Some(PathBuf::from(value("--snapshot")?)),
            "--size" => out.size = fs_components_app::parse_size(&value("--size")?)?,
            "--date" => {
                let text = value("--date")?;
                let day = Day::parse(&text);
                let day = day.filter(|d| (FIRST_YEAR..=LAST_YEAR).contains(&d.year()));
                out.date = Some(day.ok_or_else(|| {
                    fill(
                        cli::BAD_DATE,
                        &[
                            ("first", &FIRST_YEAR.to_string()),
                            ("last", &LAST_YEAR.to_string()),
                            ("given", &text),
                        ],
                    )
                })?);
            }
            "--zone" => out.zone = Some(value("--zone")?),
            "--view" => out.view = Some(view(&value("--view")?)?),
            "--week-start" => out.week_start = week_start(&value("--week-start")?)?,
            "-h" | "--help" => out.help = true,
            other => return Err(fill(cli::UNKNOWN_OPTION, &[("option", other)])),
        }
    }
    if !out.export_sensitive.is_empty() && out.export.is_none() {
        return Err(cli::SENSITIVE_NEEDS_EXPORT.to_string());
    }
    Ok(out)
}

fn view(text: &str) -> Result<ViewRange, String> {
    match text {
        "month" => Ok(ViewRange::Month),
        "week" => Ok(ViewRange::Week),
        "day" => Ok(ViewRange::Day),
        "agenda" => Ok(ViewRange::Agenda),
        other => Err(fill(cli::BAD_VIEW, &[("given", other)])),
    }
}

fn week_start(text: &str) -> Result<u8, String> {
    match text {
        "monday" => Ok(0),
        "saturday" => Ok(5),
        "sunday" => Ok(6),
        other => Err(fill(cli::BAD_WEEK_START, &[("given", other)])),
    }
}
