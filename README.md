# FreeSynergy Calendar — be somewhere at some time

**Your time belongs to you, and it should stay correct when the clocks change.**

FreeSynergy Calendar keeps your events, including the ones that repeat, with the right time zone, and shows them by month, week, day or as an agenda. You can invite people and answer invitations. Your tasks show up beside your events.

What is different here: events follow the open JSCalendar standard (RFC 8984), and this program owns it. Events import and export as iCalendar and as JSCalendar JSON, and the original file text is kept for every event. An event in New York stays at the same wall-clock time across daylight saving. If a file holds a time zone we cannot read, we refuse that event rather than silently store it at the wrong time. Tasks are not copied in; the calendar asks the Tasks program for them.

## What it can do today

Latest tag: `v0.0.2`; branch `r1-admin` is the next version (0.0.3, not tagged).

- Create, edit, delete and list events; find what happens in a time range.
- Repeating events (daily to yearly, "second Tuesday", count or until), with single occurrences changed, removed or added.
- Time zones from the newest tz database available (2026e on the machine used for testing).
- Invitations: invite, accept, tentatively accept, decline; the event's sequence number moves when it should.
- Import and export iCalendar (events, repeat rules, exceptions, attendees, alarms) and JSCalendar JSON.
- An agenda that mixes events and tasks (tasks come through the Tasks interface).
- Tell other programs when an event changes.
- Events marked sensitive are left out of exports unless released by name.
- A small server for all of this over a local socket.
- A window with the month, week and day grid and a list and forms made from the description of `Event` (create, change, delete, search, invite, respond, history, check, import, export). Commands that are not there yet are listed in the description as not supported.
- A type description of `Event` that the server hands out (`Events.types`), so any generated frontend can use it.

## What is not there yet

- The window cannot yet edit repeat rules or single occurrences, set alerts, or show several calendars. No dragging. A typed sentence, several time zones side by side, free-slot links, booking pages, placing tasks into free time and flexible blocks are listed as "not yet" and answer so.
- The window reads the data directly instead of talking to the server.
- The calendar server is not yet connected to a task server, so tasks found in an imported file are reported as not imported.
- No CalDAV, no sending invitations by e-mail (iTIP), no free/busy calculation.
- Importing is slow for large files (about 45 seconds for 4,000 events).
- Some late corrections to the core have been tested but not yet reviewed by a second reader. The DESIGN file says which.
- The interface is a draft and has no consumer yet.

## Part of FreeSynergy

Calendar sits in the groupware suite next to people, wiki, maps and messenger, and it shows tasks from FreeSynergy Tasks. FreeSynergy is a family of programs that are free for everyone, forever, and that keep your data yours. The documentation, the manifesto and the overall plan live in [fs-documentation](https://github.com/FreeSynergy/fs-documentation); the reading version is at <https://freesynergy.github.io/fs-documentation/>. Start with the [manifesto](https://freesynergy.github.io/fs-documentation/manifesto/) if you want to know why we do this.


## For developers

- One program, one command (ADR 0063): `fs-calendar` opens the window, `fs-calendar serve` serves the contract over a Unix socket, `fs-calendar list|show|add|delete|export|import` is the command line. Build without the window: `--no-default-features`.
- Workspace crates: `fs-calendar` (the binary), `fs-calendar-core`, `fs-calendar-server`, `fs-calendar-cli` and `fs-calendar-app` (libraries behind the subcommands), `fs-calendar-tasks`.
- The contract is `events.capnp` in [fs-interfaces](https://github.com/FreeSynergy/fs-interfaces) (draft). Calendar owns it. Shared JSCalendar parts: [fs-jscalendar](https://github.com/FreeSynergy/fs-jscalendar).
- Build and test: `cargo build --workspace` and `cargo test --workspace`. Try the core: `cargo run -p fs-calendar-core --example demo`.
- Design and the full state table: [DESIGN.md](DESIGN.md).
- Licence: MIT OR Apache-2.0.
