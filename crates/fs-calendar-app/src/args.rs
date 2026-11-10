//! The command line of the window.
//!
//! Pattern: Builder-like parser — plain `std::env::args`. Import and export are the commands of `fs-calendar-cli`,
//! not repeated here.

use std::path::PathBuf;

/// What was asked for.
#[derive(Debug, PartialEq, Eq)]
pub struct Args {
    /// The store file; `None` = `objects.db` in the data folder of the program (the file `fs-calendar serve` uses).
    pub store: Option<PathBuf>,
    /// Draw a picture of the window to this file instead of opening it.
    pub snapshot: Option<PathBuf>,
    /// Window or picture size.
    pub size: (u32, u32),
    /// The time zone shown; `None` = the system's.
    pub zone: Option<String>,
    /// `--help` was given.
    pub help: bool,
}

/// Parse `args` (without the program name).
///
/// # Errors
/// A message for the person: an unknown option, a missing value or a bad size.
pub fn parse(args: &[String]) -> Result<Args, String> {
    let mut out = Args {
        store: None,
        snapshot: None,
        size: (1300, 800),
        zone: None,
        help: false,
    };
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        let mut value = |name: &str| {
            it.next()
                .cloned()
                .ok_or_else(|| format!("{name} needs a value"))
        };
        match arg.as_str() {
            "--store" => out.store = Some(PathBuf::from(value("--store")?)),
            "--snapshot" => out.snapshot = Some(PathBuf::from(value("--snapshot")?)),
            "--size" => out.size = fs_components_app::parse_size(&value("--size")?)?,
            "--zone" => out.zone = Some(value("--zone")?),
            "-h" | "--help" => out.help = true,
            other => return Err(format!("unknown option {other}")),
        }
    }
    Ok(out)
}

/// The help text.
pub const HELP: &str = "fs-calendar app: the window of the calendar program

  --store <file>     the store file (default: objects.db in $XDG_DATA_HOME/freesynergy/calendar)
  --snapshot <png>   draw a picture instead of opening a window
  --size <WxH>       window or picture size (default 1300x800)
  --zone <Area/City> the time zone shown (default: the system's)

Import and export are commands of the command line: fs-calendar import, fs-calendar export.";
