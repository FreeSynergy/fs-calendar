//! The words of the command line.
//!
//! Pattern: Command (pure parse) — no I/O, so every spelling is tested without a server.

use std::path::PathBuf;

/// What to do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    /// All events, one line each.
    List,
    /// One event as JSCalendar.
    Show(String),
    /// A new event.
    Add {
        /// The title.
        title: String,
        /// Local start, `2026-12-01T10:00:00`.
        start: String,
        /// IANA zone of `start`; default `Etc/UTC`.
        zone: Option<String>,
        /// ISO 8601 duration, `PT1H`; default `PT1H`.
        duration: Option<String>,
    },
    /// Delete an event.
    Delete(String),
    /// iCalendar of all events.
    Export,
    /// Import a `.ics` file.
    Import(PathBuf),
}

/// The help text.
pub const HELP: &str = "fs-calendar                    the window (needs the gui feature)
fs-calendar app [OPTIONS]     the window with options
fs-calendar serve [OPTIONS]   the server (--socket, --data, --actor, --instance)
fs-calendar [--socket PATH] <command>

Commands:
  list                       all events
  show <id>                  one event as JSCalendar
  add <title> --start LOCAL [--zone ZONE] [--duration PT1H]
                             a new event; LOCAL like 2026-12-01T10:00:00, ZONE like Europe/Berlin
  delete <id>                delete an event
  export                     all events as iCalendar
  import <file.ics>          import VEVENTs

The socket defaults to $XDG_RUNTIME_DIR/freesynergy/calendar.sock.";

/// The words, parsed.
#[derive(Debug, PartialEq, Eq)]
pub struct Args {
    /// `--socket`.
    pub socket: Option<PathBuf>,
    /// The command; `None` for `--help`.
    pub command: Option<Command>,
}

/// Parse the words after the program name.
///
/// # Errors
/// An unknown word, a missing value, a missing id. The message says what is wrong.
pub fn parse(words: &[String]) -> Result<Args, String> {
    let mut socket = None;
    let mut rest: Vec<&str> = vec![];
    let mut it = words.iter().map(String::as_str);
    while let Some(w) = it.next() {
        match w {
            "--help" | "-h" => {
                return Ok(Args {
                    socket,
                    command: None,
                });
            }
            "--socket" => socket = Some(it.next().ok_or("--socket needs a path")?.into()),
            other => rest.push(other),
        }
    }
    let (name, tail) = rest.split_first().ok_or("no command; try --help")?;
    let one = |what: &str| -> Result<String, String> {
        match tail {
            [x] => Ok((*x).into()),
            _ => Err(format!("{name} needs exactly one {what}")),
        }
    };
    let none = |c: Command| -> Result<Command, String> {
        if tail.is_empty() {
            Ok(c)
        } else {
            Err(format!("{name} takes no argument"))
        }
    };
    let command = match *name {
        "list" => none(Command::List)?,
        "export" => none(Command::Export)?,
        "show" => Command::Show(one("id")?),
        "delete" => Command::Delete(one("id")?),
        "import" => Command::Import(one("file")?.into()),
        "add" => add(tail)?,
        other => return Err(format!("unknown command {other}; try --help")),
    };
    Ok(Args {
        socket,
        command: Some(command),
    })
}

fn add(tail: &[&str]) -> Result<Command, String> {
    let (title, mut it) = match tail.split_first() {
        Some((t, rest)) if !t.starts_with("--") => (*t, rest.iter()),
        _ => return Err("add needs a title".into()),
    };
    let (mut start, mut zone, mut duration) = (None, None, None);
    while let Some(flag) = it.next() {
        let slot = match *flag {
            "--start" => &mut start,
            "--zone" => &mut zone,
            "--duration" => &mut duration,
            other => return Err(format!("unknown option {other} for add")),
        };
        *slot = Some((*it.next().ok_or(format!("{flag} needs a value"))?).to_string());
    }
    Ok(Command::Add {
        title: title.into(),
        start: start.ok_or("add needs --start LOCAL")?,
        zone,
        duration,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(s: &str) -> Vec<String> {
        s.split_whitespace().map(String::from).collect()
    }

    #[test]
    fn commands_and_the_socket_option_in_any_place() {
        let a = parse(&words("--socket /x/y list")).unwrap();
        assert_eq!(a.socket, Some("/x/y".into()));
        assert_eq!(a.command, Some(Command::List));
        assert_eq!(
            parse(&words(
                "add Talk --start 2026-12-01T10:00:00 --zone Europe/Berlin"
            ))
            .unwrap()
            .command,
            Some(Command::Add {
                title: "Talk".into(),
                start: "2026-12-01T10:00:00".into(),
                zone: Some("Europe/Berlin".into()),
                duration: None,
            })
        );
        assert_eq!(parse(&words("-h")).unwrap().command, None);
    }

    #[test]
    fn mistakes_are_named() {
        for bad in [
            "",
            "frob",
            "show",
            "show a b",
            "add",
            "add T",
            "add --start x",
            "add T --start",
            "add T --start x --y 1",
            "list x",
            "--socket",
        ] {
            assert!(parse(&words(bad)).is_err(), "{bad:?}");
        }
    }
}
