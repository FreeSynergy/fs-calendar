//! The command line.
//!
//! Pattern: Builder-like parser — plain `std::env::args`, no extra crate for a few options.

use std::path::PathBuf;

use fs_components_workbenches::calendar::{Day, FIRST_YEAR, LAST_YEAR, ViewRange};

/// What was asked for on the command line.
#[derive(Debug, PartialEq, Eq)]
pub struct Args {
    /// Store file; `None` = in memory, gone when the program ends.
    pub store: Option<PathBuf>,
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
            store: None,
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
                .ok_or_else(|| format!("{name} needs a value"))
        };
        match arg.as_str() {
            "--store" => out.store = Some(PathBuf::from(value("--store")?)),
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
                    format!("--date wants YYYY-MM-DD in the years {FIRST_YEAR} to {LAST_YEAR}, not {text}")
                })?);
            }
            "--zone" => out.zone = Some(value("--zone")?),
            "--view" => out.view = Some(view(&value("--view")?)?),
            "--week-start" => out.week_start = week_start(&value("--week-start")?)?,
            "-h" | "--help" => out.help = true,
            other => return Err(format!("unknown option {other}")),
        }
    }
    if !out.export_sensitive.is_empty() && out.export.is_none() {
        return Err("--export-sensitive needs --export".into());
    }
    Ok(out)
}

fn view(text: &str) -> Result<ViewRange, String> {
    match text {
        "month" => Ok(ViewRange::Month),
        "week" => Ok(ViewRange::Week),
        "day" => Ok(ViewRange::Day),
        "agenda" => Ok(ViewRange::Agenda),
        other => Err(format!(
            "--view wants month, week, day or agenda, not {other}"
        )),
    }
}

fn week_start(text: &str) -> Result<u8, String> {
    match text {
        "monday" => Ok(0),
        "saturday" => Ok(5),
        "sunday" => Ok(6),
        other => Err(format!(
            "--week-start wants monday, saturday or sunday, not {other}"
        )),
    }
}

/// The help text.
pub const HELP: &str = "fs-calendar app: the window of the calendar program (options of the window)

  --store <file>     keep the events in this store file (default: in memory, gone at the end)
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
