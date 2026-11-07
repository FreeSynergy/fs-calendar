# fs-calendar – Design

`fs-calendar` is the calendar program of FreeSynergy. This repo holds the **program core** (`crates/fs-calendar-core`, no GUI), the **server** (`crates/fs-calendar-server`, the contract over a Unix socket) and its frontend (`crates/fs-calendar-app`).
Events are **JSCalendar 2.0** objects (`draft-ietf-calext-jscalendarbis-22`) inside AS2 envelopes stored through
`fs-store` (ADR 0014, 0019, 0020). Every program is at least a workbench (ADR 0037); the work surface is the workbench `calendar` in `fs-components`, which embeds the `calendar-view` widget (ADR 0005) and knows neither this core nor its types; `fs-calendar-app` connects the two.

## State

Version 0.4.1 (not tagged; branch `server`). Dependencies are git tags: `fs-interfaces` v0.35.0, `fs-tasks-core` v0.5.3, `fs-jscalendar` v0.1.3, `fs-store` v0.2.0. This version adds the Cap'n Proto server `fs-calendar-server` (contract `events.capnp`, every method, `subscribe` included) and closes the core faults the frontend had found (list below). Before: `Calendar` follows the provider rule (ADR 0045): every method of `events.capnp`, defaults answering `Unsupported`, `capabilities()` returns `Capabilities`, and `import_text` / `export_text` by media type. `Event` itself stays here (ADR 0050).

**Task moved out (ADR 0050).** `Task` is owned by `fs-tasks`; this repo has no Task model, validation, upgrade, queries or VTODO code any more.
The calendar depends on the **`TaskItems` trait** of `fs-tasks-core` (git tag `v0.5.3`) and shows tasks only through it
(`CalendarRepo::with_tasks`, `agenda`). `CalendarObject` (the Event-or-Task enum) is gone: the API takes `Event`.


| Area | State |
|---|---|
| Typed `Event` (serde), lossless unknown/vendor properties (`extra` on every typed object) | done; the 5 demo events round-trip equal as JSON |
| Validation (`version` "2.0", `uid`, `start`, LocalDateTime without zone suffix/fraction, IANA zones only, durations, status/privacy/freeBusy/progress enums, single `recurrenceRule`, count XOR until, override keys, obsolete 1.0 properties rejected) | done |
| `upgrade()` RFC 8984 -> 2.0: `recurrenceRules[0]` -> `recurrenceRule` (rest to `freesynergy.net:legacy`, with a note), obsolete properties moved to `freesynergy.net:legacy`, `Z` suffix -> `Etc/UTC`, fractions stripped, custom zones moved out; idempotent; Groups | done |
| Contract `Calendar` (trait, object-safe) + store-backed `CalendarRepo` (create / get / update / delete / list / queries / invitations / import / export); depends on `dyn ObjectRepository` + `dyn ObjectQuery` + `dyn ActivityLog` only | done |
| Recurrence engine behind `RecurrenceEngine` (default `RRuleEngine` on `rrule`; `CalendarRepo::with_recurrence_engine`) | done |
| Formats behind `Importer` / `Exporter` (`IcsFormat`, `JsCalendarFormat`); `CalendarRepo::import(&dyn Importer, ..)` / `export(&dyn Exporter, ..)` | done |
| Invitation flow as State: `ParticipationStatus` + `Response`, transition `respond`; `invite`/`respond` write via `put_and_log` | done |
| Time-zone source as Strategy: `ZoneSource` (`SystemTzdb`, `BundledTzdb`), `time::newest` picks, `Zone::parse_in` for an explicit source | done |
| Envelope: type `Event`, `name` = `title` (ADR 0032, placeholder if untitled), `context=calendar`, IRI `https://<instance>/events/<id>`, **`startTime` / `endTime`** (first start; end of the last instance, `9999-12-31T23:59:59Z` if unbounded) | done |
| Index fields (`start_utc` / `end_utc` are kept next to the envelope times for compatibility and for `migrate_time_fields`): `uid`, `type`, `start_utc`, `end_utc`, `due_utc`, `recurring`, `status`, `progress`, `sequence`, `participant_email`, `calendar_id`, `tzdb_version` (imports) | done |
| Time-range query `occurrences(from, to)` / `upcoming(now, days)` | done; candidates selected **by the store** (`start_time < to AND end_time >= from` on the envelope), only those are expanded; `tasks_due` = store range on `due_utc`; `list_events` / `list_tasks` ordered by the store. `migrate_time_fields()` fills the envelope times of objects written before fs-store 0.2 |
| Recurrence expansion at query time: FREQ DAILY/WEEKLY/MONTHLY/YEARLY (and the rest `rrule` supports), INTERVAL, COUNT, UNTIL, BYDAY (incl. nth), BYMONTHDAY, BYMONTH, BYSETPOS, ...; overrides: excluded, patched (PatchObject paths), extra instances; wall-clock time kept across DST | done |
| Series end for the index (`end_utc` of the last instance, `9999-12-31T23:59:59Z` if unbounded) | done |
| Invitations as log activities: `invite` (`Invite`), `respond` (`Accept` / `TentativeAccept` / `Reject`) update `participationStatus` and log, **in one transaction** (`put_and_log`, fault-injection test); `invitation_log` (also reads 0.1 entries) | done, real AS2 kinds |
| Sequence handling: `sequence` + 1 when start/zone/duration/rule/status change | done |
| Tasks in an import go to `fs-tasks` in every format (`Parsed::tasks`, `VTODO` with its original); without a task source the report notes each one, nothing is dropped silently (ADR 0050) | done |
| iCalendar import (VEVENT incl. RRULE, EXDATE, attendees, alarms) via `calcard`; original kept per UID (`text/calendar`), tzdb version recorded | done |
| iCalendar export | done (participants without any address are left out; see below) |
| Example `cargo run -p fs-calendar-core --example demo`: import demo events, next 30 days, accept an invitation, export | done |
| Time zones | `jiff` with the newest tzdb available: system copy (`$TZDIR`, `/usr/share/zoneinfo`, version from `tzdata.zi`) if not older than the bundled `jiff-tzdb` copy, else the bundled one. Here: **tzdb 2026e** from `/usr/share/zoneinfo` (bundled copy is also 2026e). Version recorded as index `tzdb_version` with each imported original | done |
| Tasks in calendar views | done: `CalendarRepo::with_tasks(Arc<dyn TaskItems>)`, `agenda(from, to)` = event instances + tasks due (queried through `TaskItems::query`); `import_ics` hands `VTODO`s to the attached task source, ignores them otherwise; `export_ics` is events only |
| Provider rule: `Error::Unsupported`; required are `capabilities()` and the reads (`get`, `get_event`, `get_by_uid`, `list_events`, `occurrences`), everything else defaults to `Unsupported`; `CalendarRepo` reports all ids of the schema, `event.subscribe` included | done |
| `importText` / `exportText` by media type (`TextFormat`, Factory): `text/calendar` and `application/jscalendar+json` (parameters and case ignored); any other type is `Error::MediaType`, nothing is stored | done |
| JSCalendar JSON export (`JsCalendarFormat` is now also an `Exporter`): one event = the object, several = a JSON array; both read back through the importer | done, provisional (see open points) |
| Tests (older rows) | 55 at version 0.4.0: `tests/provider.rs` (2) and `tests/media.rs` (3) are new on top of the 50 below (3 new in `tests/task_forwarding.rs`; before them: the 43 event tests are unchanged; the 2 task tests and the task parts of 2 others moved to fs-tasks, 4 new in `tests/tasks.rs` cover the task interface: fake `TaskItems`, real `TaskRepo` over the same store, `.ics` with `VTODO`) (before the move: 45; 39 pre-refactor tests unchanged in behaviour, now split by feature into `tests/{model,crud,queries,tasks,invitations,import,ics,recur,recur_overrides,time}.rs`; the 6 former unit tests live in `tests/time.rs`; 6 new in `tests/interfaces.rs` and `tests/time.rs` cover the trait seams) |
| **Server** `fs-calendar-server`: every method of `Events`, `Items` and `Invitations`, `subscribe` (with `ended`), `supportReport`; errors as typed problems (`calendar/not-found` 404, `calendar/conflict` 409, `calendar/invalid` 422, `Unsupported` 501, `too-large` 413); binary with `--socket`, `--data`, `--actor`, `--instance` (must be a domain, else exit code 2). No task program is attached, so an imported `VTODO` is noted in the report | done, draft; pages cut from the whole list; `occurrences` is one answer and refused as `too-large` above a page |
| Identity of a `uid` (`uid::normalise_uid`, index `uid_key`): `urn:uuid:X` and `X` (any case, hyphens or not) are one event for `create`, `get_by_uid`, import and `update`; a UUID that is another object's id is a conflict, never an overwrite | done, `tests/uid.rs` |
| Unknown `TZID` on import: the event is **refused** with the zone's name in `failed` (it would mean another time, and an unknown start with a known end would store a length of zero). A name `calcard` itself resolves (alias, `VTIMEZONE`) is no loss | done, `tests/zones.rs`, provisional (see open points) |
| Sensitive marking (ADR 0029): `freesynergy.net:sensitive` anywhere in the event gives `envelope.sensitive` (categories, sorted; `freesynergy.net:unclear` if none is readable); an update never takes a category away; a store that is not encrypted refuses the event. `privacy` alone marks nothing | done, `tests/marks.rs` |
| Series start: the envelope and the index start at the **earliest** instance, so an override or extra instance before the master start is found in its week | done, `tests/early.rs` |
| A broken override costs itself: written, it is refused (each patch is applied and the instance validated); read, `Calendar::window` returns the instances and names (`Damaged`) what could not be built, also an event whose rule or zone does not read. `occurrences` / `agenda` never fail for that | done, `tests/damage.rs` |
| Compare-and-set: `Calendar::update_checked(id, &Revision, event)`; `Revision` (64 bits) = fingerprint of the store's `updated` and the content; over the wire `Items.updateIf` and `Event.revision`. Public writes take turns (`writes`), so check and write are one step; of two writers on one revision exactly one wins | done, `tests/revision.rs` |
| Limits (`limits.rs`), each one tested. **Event:** JSON + name at most 16 MiB; at most 100 000 values, 32 levels and 40 MiB of estimated memory (`memory_weight`: 32 bytes a value; 656 bytes a leaf of an object, one per five entries; 24 + 16 bytes and twice the text for a key; 16 bytes and twice the text for a string, because reading one with escapes decodes it in a buffer of its size besides the string that is kept; an array 128 bytes and 64 bytes more for each element, three times its elements at the highest, because it grows by doubling and is then shrunk while it is read; `tests/weight.rs` proves it is above what a copy really takes, and the estimate **plus the bytes of the input** is above the peak of reading the text with `parse_bounded` (a claim of that size only, not of the estimate alone), for objects of one entry, chains of depth 10, arrays of one element, chains of arrays, small numbers, text with escapes, two byte characters, keys with escapes and many keys). **They are checked while the text is read** (`model::parse_bounded`, a serde Visitor that builds the `Value` and stops at the first value past a limit, so a text of millions of small values never gets built: a 6.8 MB document stops at 23 MB, not 1.3 GB) in the server (`wire.rs`) and in the JSON importer, and again on the value in `build` (one copy serves size, memory, marking and store). The server lets at most 4 requests that write events run at once (`MAX_CONCURRENT_WRITES`), each holds its parsed event while it waits for the store. **Answer:** the instances of all events weigh at most 33 MiB by the same estimate plus 256 bytes each (counted before an `Event` is built; every candidate start, overridden or not, is a time span until it is returned; the patch of an override is read for `start`, `timeZone`, `duration` only); at most 10 000 instances of one event; every cut is told, and the server answers a cut with `too-large` (413) instead of a shorter list (the contract has no place to say "cut"). The candidates are read from the store a page at a time, ordered by the event's own id kept as the index `page_key` (lower case and unique, so the store sorts and compares it alike; the cursor is `page_key > last`; the first page is one event, then as many as fit 16 MiB by the heaviest of the page before). Every event written by this version has a `page_key`; an event without one would be missing from windows until `migrate_time_fields` rewrites it, which it does for every object without a `page_key`. **Which cut is a cut:** one event that is too much for the window by itself (more than 10 000 instances, more than 65 535 starts in a range, or more than the answer budget less a reserve of 1 MiB) is **named alone** (`Damaged`, `cut: false`, its id, also on stderr) and the rest answers; a `cut` (`cut: true`, the server's 413) is only what the good events together put over the budget of the answer. **Measured upper bound** of a question (`tests/memory_nodes.rs`, counting allocator; claimed: three times the answer budget plus three copies of the heaviest event, the store's own memory aside; the figures are the last run): objects of one entry (30 000, 23 MiB estimated) 44 MiB to write and 77 MiB to ask; chains of depth 10 39 and 80; 90 000 small numbers 10 and 37; 49 000 arrays of one element 14 and 41; 3 000 chains of arrays 14 and 42; 40 events of 10 000 small objects each, a month: 61 MiB; a master of 4 MB with 2000 overrides 41 and 28 (`tests/memory_overrides.rs`); a daily series of 150 years of a 20 KB event holds the budget (`tests/memory.rs`). **Not bounded by this crate:** `fs-store` v0.2.0 keeps about 6 MiB for each distinct page query of rows this heavy (sixty queries of one row each hold 330 MiB, measured without this crate), so 40 events of 20 MiB estimated each, a month, take 169 MiB (the test adds 6 MiB per event for it); it is the store's memory, reported to the strategy (statements with the key or offset written into the SQL text, apparently kept). **Rules:** a rule whose one period can give more than 100 000 starts, or one without `count` and `until` that gives more than 320 starts a day (every five minutes are 288; every four are 360), is refused when written (`Error::RuleTooExpensive`, problem `calendar/rule-too-expensive`, 422). 320 a day are 9 920 instances in 31 days, below the 10 000 of one event, so a legal series without an end is not cut by the number of instances in a month window (tested for every five minutes: 8 928 instances); whether its instances fit the byte budget depends on the size of the event, and a series with an end or a `count` can still be too much for a window, which is then named. Nothing promises that every series that can be written can be answered for every window. When read, a window is walked **from its own start** only where that gives the same instances as walking from the series start: `daily` and `weekly` rules, and `hourly`, `minutely`, `secondly` rules **without any `BY` part**, all without `count` and without `bySetPosition`, and never behind the rule's `until` (the start is shifted by whole periods of the interval). The sub-daily rules with `BY` filters are not shifted because the engine aligns those to the day, not to the start (a minutely rule every 7 minutes in hours 2 and 3 gave other minutes from other windows), and `bySetPosition` depends on where the walk begins. `tests/jump.rs` compares 400 random rules and windows (fixed seed) and the two reported rules with the engine from the real start. `monthly`, `yearly` and every rule not shifted are walked from their real start, estimated at most 500 000 starts, and the engine itself stops after 500 000 dates (tested on the engine directly), so a sub-daily rule with `BY` parts that is allowed (up to 320 starts a day) becomes unanswerable after a few years and is then named alone. A rule the estimate or the engine refuses, and any event that is too much for the window by itself (older data included, also with an `until`), is **named alone**: one series never makes the rest of the calendar unanswerable. An edit of an event whose rule is unchanged is not refused when the end of its series cannot be computed: it is stored open-ended (a candidate for every window from its start) and checked at every read. A rule that never hits (`byMonth 2` with `byMonthDay 31`) ends by the engine's own bound, about 150 to 330 ms per event and question. A start in years 1 to 9998; index times stay in years 0 to 9999; times at the edge of the time type are clamped, never a panic; values in messages are quoted short | done, `tests/{limits,years,memory*,weight,bounded,density,extremes,unbounded}.rs`, server `tests/{limits,cut,quoting,concurrent}.rs` |
| An import says what it could not read: `DTSTART`, `DTEND`, `DURATION`, `RRULE` of the `VEVENT`'s own unfolded lines, no false alarm from an alarm or a zone | done, `tests/unread.rs` |
| `event.subscribe`: `CalendarFeed` (Observer), `Created` / `Updated` / `Deleted` for create, update, invite, respond, delete, import | done, `tests/feed.rs` |
| Tests now | core 128, server 38 (a real socket, values compared, exact problem types, the real binary across a restart, cancel at a real step), app 92 |
| ADR 0043 refactor | done: every file <= 200 lines (`check_rust_rules.py`: 0 errors, 0 warnings), `//! Pattern:` in every module, `#![deny(missing_docs)]`, `#![warn(clippy::pedantic)]` clean |

## Frontend `fs-calendar-app` (first version, provisional)

Imports no `blinc_*` crate (ADR 0009). Arguments: `--store <file>` (default in memory), `--import <file.ics>`, `--export <file>`, `--export-sensitive <uid>` (repeatable), `--snapshot <png>`, `--size WxH`, `--date YYYY-MM-DD` (fixes today, at noon, for pictures and tests), `--zone Area/City` (default `TZ`, then `/etc/localtime`, then UTC), `--view month|week|day|agenda`, `--week-start monday|saturday|sunday`.

**In process, not RPC (provisional, do not forget):** `CalendarAdapter` holds `Arc<dyn Calendar>` in the same process and drives it with a tokio handle, blocking the interface thread. When the Cap'n Proto server of `events.capnp` exists, the adapter must call it instead. Until then the program opens the store itself, and the workbench's calls freeze the window for as long as the core takes (see far future below).

| Area | State |
|---|---|
| Month, week, day, agenda (the `calendar-view` widget, nothing drawn here); today, previous, next; a click reports a day or an item | done |
| Times: an event in a zone is converted to the zone shown; an all-day or floating event keeps its local date and clock, wherever it is shown. The core is asked for the days shown plus two days each side, because it places those by its own floating zone, and what does not fall on the days by the wall clock is dropped. Never more than a view shows: at most 42 + 4 days | done, `tests/times.rs` |
| Create, edit (title, start, end, all-day, time zone, location, description), delete with two separate actions in two places | done |
| Editing keeps everything else: the form goes onto the event as stored now, and only fields that differ from what the form started with are written. Proved by comparing the whole event as JSON in `tests/keeps.rs`; the store keeps the original | done |
| End of a timed event: the end the person sees is the end stored. Moving the start alone shortens or lengthens it; the duration is rewritten in exact hours, measured across a clock change. An all-day end is the last day covered | done, provisional |
| Series, shown through `occurrences`. A click opens the occurrence; Edit opens the **stored event**, never the occurrence, so an override cannot be written over the series. Title, location and description change for all occurrences; start, end, all-day and zone are locked in the form and refused by `save`, because moving a series leaves its override keys behind and an excluded occurrence comes back. Delete says it takes the whole series. An occurrence with values of its own keeps them; its sheet says so and names the series' title | done |
| Marks (ADR 0029): an event with `envelope.sensitive`, the key `freesynergy.net:sensitive` anywhere in the content, or a JSCalendar `privacy` other than `public` is marked in every view, on the sheet and in the form. Nothing opens by itself. A save keeps the marks because it does not touch them, and the notice after a delete does not repeat a marked title | done, the rule is provisional |
| iCalendar import at start; export of all events, written through a temporary file beside the target and never over an existing one, then the program stops (with `--snapshot` it draws as well). Marked events are left out and named; `--export-sensitive <uid>` releases one; naming nothing writes nothing | done |
| Tasks: shown read-only through `Calendar::agenda`, that is the `TaskItems` interface of `fs-tasks-core`, at the time they are due, marked as a task; the sheet says they belong to the tasks program. `main` attaches a `TaskRepo` over the same store (ADR 0050) | done |
| One bad event costs itself: if the agenda fails as a whole, also by a panic of the time library, the adapter expands event by event, names what is damaged above the calendar (a click opens it, it can be deleted) and reads the tasks on their own, through `TaskItems`. Something marked is not called by its title there and carries its mark. A failure of the task source is said as that. A damaged series shows no occurrence at all until it is dealt with. Beyond three lines the rest are only counted | done, `tests/damage.rs` |
| An override moved before its series' first start, or an extra occurrence given there: the store does not offer it there, so at each reading such series are found in `list_events` and expanded in the adapter; a click finds the occurrence from the event itself. Nothing is remembered, so a change elsewhere is right at the next reading. Cost of a reading among 5000 events: 110 ms in a release build, 1.3 s in a debug build, on the interface thread (`looking_for_early_series…`, ignored by default) | done, provisional until the envelope starts at the earliest occurrence |
| A save after a conflict writes only the fields the person changed, on the event as it is now. Start, end, all-day and zone belong together: if both sides changed any of them the save is refused, says what changed elsewhere, keeps the draft and does not offer to write anyway; nothing is written until the person's times are put back, or Cancel and again from the current times. Other fields merge | done, `tests/conflicts.rs` |
| Clock edges: a time the clocks skip is refused, a start in that hour stored by someone else reads as the time drawn (03:15, not 02:15); an event whose end reads earlier than its start, in the repeated hour, is drawn with its real length, and sheet and form say why; a zone-only change keeps both clock times, measuring the length again only if it must. Typed years outside 1 to 9000 are refused | done, `tests/clock_edges.rs`, `tests/far_years.rs` |
| Import goes part by part, so one that the time library cannot hold (the panic of `fs-jscalendar` v0.1.0, fixed in v0.1.1, which needs a core `Cargo.toml` change) costs itself, named by `uid`, and the rest arrive; tasks go on to `fs-tasks` one by one. It names events it stored whose `TZID` no zone answers to, on any start, end or override, read with calcard, the core's own parser, until the core says so; a name calcard maps to a real zone is no loss. What is skipped is not named, what is marked not by its title | done, provisional, `tests/import_names.rs` |
| A released event leaves the file `CLASS:PRIVATE` with its categories, also if it was marked only on its envelope, and comes back marked; such a file is mode 0600 from the moment it exists. A bad `TZ` is said in the status line, and times are in UTC | done |
| Menus (ADR 0056): Event: New; View: Month, Week, Day, Agenda; Go: Today, Previous, Next. They do what the buttons do | done, `tests/menus.rs` |
| Windows | `launch` is wired, never opened by tests; `AppWidget::build` and `picture` are drawn offscreen and compared |

**What is drawn is as of the last reading.** Nothing hears of a change by someone else: it shows after the next click on the item or move of the view.

**Listed, not fixed:** an event of 01:45 for `PT2H` on the night clocks go back draws one hour, what the clock shows, and its agenda line says so too (real length: two hours). The export report does name marked events it left out or released by title, because it must say which (the original rule of this frontend).

**Unsupported in this version** (the workbench offers none of it; each is possible later): changing one occurrence, deleting one occurrence, and creating or editing a recurrence rule; moving a whole series; invitations and replies (the core has them, the frontend does not show them); alerts, virtual locations, links, keywords, priority, free/busy, status (kept and named on the sheet, not editable); several calendars; drag, resize, swipe, pinch; the three-day week on small screens; printing; settings kept across runs; `event.subscribe`, so a change made by another program shows only after the next navigation.

Decisions (provisional, easy to reverse):

* `privacy: private` and `secret` count as sensitive. So `CLASS:PRIVATE` from a file stays out of an export, by name.
* The key `freesynergy.net:sensitive` is what `fs-people` uses; the core has no marking of events and does not move it onto the envelope. So a marked event is not refused by an unencrypted store (a wish below).
* Week starts on Monday unless `--week-start` says otherwise; times are 24-hour in the widget.
* A new event starts at nine for an hour in the zone shown, on the day chosen last or the day the view is around.
* The form's `InputSlots` and the harness of the workbench tests are still copied between workbenches in `fs-components`. `write_new_file` and `parse_size` are shared now, in `fs-components-app`.

**The wishes the frontend had for the core are done in the core** (this version): compare-and-set (`update_checked`), unknown `TZID` refused by name, the rule engine's cost and its 65 535 starts bounded and named, the envelope starting at the earliest instance, `TaskEntry` with its envelope (fs-tasks v0.5.3), a broken override costing itself, the marking on the envelope. The frontend still carries its provisional workarounds (the fallback to single events, its own zone check and its early-series search); they stay harmless and go when Thread 4 removes them (see "What the frontend must change").

## What the frontend must change (reported to the strategy, not done here)

* Remove its own `fs-tasks-core` dependency: the calendar passes the types of v0.5.3 through (`Agenda.tasks`), and the app's `Cargo.toml` tag was moved from v0.4.1 to v0.5.3 here only so the workspace builds. Talk to the server and, for tasks, to `fs-tasks-server`, both over RPC.
* `Agenda` has a new field `damaged`, and `Calendar::window` names what could not be answered. A non-empty `damaged` is handled here as a failed agenda (event by event) so the old behaviour stays; the app should show `damaged` instead and drop its own expansion fallback.
* An unknown `TZID` now arrives in `ImportReport.failed` (`uid: iCalendar: the time zone "X" is unknown ...`), the event is not stored. `lost_zones.rs` and `ImportSummary::zone_lost` are dead weight; four tests in `import_names.rs` were adapted to the new answer.
* The early-series search (`early.rs`) is not needed any more: the store finds a series in the week of its earliest instance.
* The far-end test asks year 3300 instead of 8999: the core refuses windows further than 500 000 rule steps from the series start (named in `Window.damaged`).
* Broken overrides cannot be written through the core any more; the tests that need one put it into the store behind the core (`World::add_json_past_core`, `put_overrides`).
* `CalendarAdapter` calls the core in process; it must call `fs-calendar-server` instead (`Items.update` has no revision: see open points).

## Structure and pattern map (ADR 0043)

Public contract: the trait `Calendar`; `CalendarRepo` is its implementation (Repository + Facade) and keeps all its
inherent methods (`create` / `update` take `Event`), so callers that do not import the trait
keep working. Every file is one concept, <= 200 lines (tests included); `lib.rs` and every `mod.rs` only declare and re-export.

| Module | Pattern | Why |
|---|---|---|
| `calendar.rs` | Repository (interface) + Null Object (`Unsupported` defaults) | the program's public contract; GUI, hub and tests use `dyn Calendar` |
| `repo/agenda.rs` | Facade over two interfaces | events from the calendar itself, tasks only via `TaskItems` of fs-tasks (ADR 0050) |
| `repo/handle.rs` | Repository + Facade | one small type hides store, envelope rules, formats and recurrence |
| `repo/backend.rs` | Adapter | `Arc<dyn Store>` presented as `dyn ObjectRepository` + `dyn ObjectQuery` + `dyn ActivityLog` |
| `repo/{crud,update,queries,migrate}.rs` | Repository / Template method | writes that take turns and tell the feed, sequence-aware update (and compare-and-set) with `put_and_log`, range queries and the `Window`, idempotent migration |
| `repo/{build,span}.rs` | Adapter / Strategy (by kind) | object to AS2 envelope + index fields; event span via the engine |
| `repo/transfer.rs` | Template method | fixed import flow (split, skip uid, parse, normalise, store, report); split / parse are the `Importer` |
| `format/media.rs` | Factory | media type -> `Importer` / `Exporter`; unknown type is an error |
| `format/*`, `ical/*` | Adapter (+ Strategy) | `Importer` / `Exporter` traits; iCalendar (through `fs-jscalendar`) and JSCalendar JSON behind them |
| `invite/status.rs` | State | `ParticipationStatus` x `Response` -> next state (single transition function) |
| `invite/flow.rs` | State (client) | `respond` / `invite` apply the transition and log in one `put_and_log` transaction |
| `invite/log.rs` | Observer-style read model | invitation history from the activity log (reads 0.1 entries too) |
| `recur/engine.rs`, `recur/rrule_engine.rs` | Strategy + Adapter | `RecurrenceEngine` yields a rule's local starts; `RRuleEngine` wraps `rrule` on wall-clock time |
| `recur/expand.rs` | Template method | overrides, exclusions, patches, zone conversion and window filter around any engine |
| `recur/patch.rs` | Command | JSCalendar `PatchObject` applied to a base value |
| `model/event.rs` | Template method (`calendar_object!` of `fs-jscalendar`) | only what is specific to `Event` |
| `validate/check.rs` | Specification | event rules; the shared rules and `Ctx` are in `fs-jscalendar` |
| `uid.rs` | Strategy (pure function) | one canonical text per identity, used when indexing, looking up and comparing |
| `feed.rs` | Observer | `CalendarFeed`, `CalendarListener`, `Registry` (subject), `Subscription` (drop = unsubscribe) |
| `revision.rs` | Newtype | the revision (`u64`) for compare-and-set; the trait method is `Calendar::update_checked`, the wire form `Items.updateIf` |
| `limits.rs`, `repo/consts.rs` | Constants | every bound, and the types and context of the objects |
| `model/marks.rs` | Iterator | the marking is allowed at any level; an iterator over the markings of the serialised event |
| `model/weight.rs` | none | a pure measure of a value: memory, values, levels |
| `model/bounded.rs` | Visitor | a serde Visitor that builds the `Value` and stops at the first value past a limit |
| `model/counter.rs` | none | the running counters of that read (mutable) |
| `recur/early.rs` | Iterator | `filter_map` / `fold` over the overrides gives the earliest instance |
| `recur/steps.rs` | Specification | pure predicates over a rule: too dense a period (refused when written), too long a walk (named when read); and the shift of the start to the window |
| `recur/candidates.rs` | Template method | collect the rule's starts, add override keys, drop exclusions, keep what overlaps the window, all as time spans; the rule is the `RecurrenceEngine` strategy |
| `recur/instance.rs` | Template method (one step of `expand`) | one instance: master, override, UTC span; a failure belongs to its key |
| `validate/overrides.rs` | Specification | each override must make a valid instance |
| `ical/zones.rs`, `ical/unread.rs` | Specification | which `TZID` no one answers to; which property the conversion left out |
| `contract/*` (feature `contract`) | Adapter | the generated code and the wire <-> Rust mapping of `Response` |
| `fs-calendar-server/{events_service,items_service,invitations_service}.rs` | Adapter + Factory | wire in, `Calendar` call, wire out; the root hands out `Items` and `Invitations`; depends on the traits only, never on `CalendarRepo` |
| `fs-calendar-server/{wire,report,errors,feed}.rs` | Adapter, Observer | events, instances and reports on the wire, core errors as RFC 9457 problems, `CalendarFeed` to the shared forwarder |
| `error.rs` | none | plain error enum; one `From` for the errors of `fs-jscalendar` |

## Migration for dependents

* **0.3.0:** `ZoneSource` and `Event::from_value` use the error of `fs-jscalendar` (the core `Error` converts from it with the same variants); `time`, `upgrade` keep their paths as re-exports.
* `CalendarRepo::store()` is removed: the repository holds only the narrow store traits. Keep your own `Arc<dyn Store>`
  for undo / log / count calls (the old tests did `r.store().undo(..)`).
* `repo::Response`, `InvitationEntry` moved to `invite` (still re-exported at the crate root).
* `time::TzDb` is replaced by the `ZoneSource` trait with `SystemTzdb` / `BundledTzdb`; `TzDb::detect/newest/bundled/from_dir`
  became `time::detect`, `time::newest`, `BundledTzdb::new`, `SystemTzdb::from_dir`. `time::tzdb_version()` / `tzdb_source()` are unchanged.
* `recur::{expand, series_end}` keep their signatures (default engine); new `expand_with` / `series_end_with` take a `&dyn RecurrenceEngine`.
  `recur::rrule_string` keeps its path.
* `CalendarRepo::new/with_instance/with_floating_zone/create/get/get_event/update/delete/list_events/occurrences/upcoming/
  invite/respond/invitation_log/import_*/export_ics`, `Event`, `Zone`, `utc`, `validate::*`, `upgrade` keep their paths
  and signatures; fs-hub (which only uses `CalendarRepo`, `Event` and `create`) compiles unchanged.
* **0.2.0 (ADR 0050): removed** `Task`, `CalendarObject`, `get_task`, `get_object`, `list_tasks`, `tasks_due`, `validate_task`, `TASK_TYPE`,
  `model::TASK_STATUS_KEY`; `create` / `update` / `Importer::parse` / `Exporter::export` now take / return `Event` instead of `CalendarObject`.
  Tasks: use `fs_tasks_core::TaskItems` (or `CalendarRepo::with_tasks` + `agenda`). Tasks written by calendar 0.1 have `context=calendar` and are
  not found by fs-tasks (`context=tasks`); no such data existed outside tests. JSCalendar JSON import reports `Task` entries as failed (not events).
* New: `Calendar`, `Importer`, `Exporter`, `RecurrenceEngine`, `ZoneSource`, `ParticipationStatus`,
  `CalendarRepo::{with_backend, with_recurrence_engine, with_tasks, agenda, import, export}`. Inherent methods win over trait methods, so importing
  `Calendar` next to `CalendarRepo` is not ambiguous.

## Crates

* Server only: `capnp` 0.27, `capnp-rpc` 0.27. Core: `tokio` (`sync`, for the write lock), `calcard` (also directly, to tell which zone a file names), `capnp` (optional, feature `contract`).
* Frontend only: `fs-components-app`, `fs-components-workbenches` (git `rev`, provisional until a tag exists).
* `fs-store` (git tag v0.2.0). `fs-interfaces` (git tag v0.35.0): `Capabilities`, `Unsupported`, the generated contract and (server, feature `rpc`) the shared server side. `fs-tasks-core` (git tag v0.5.3): the `TaskItems` trait and the task types it uses. `fs-jscalendar` (git tag v0.1.3): the shared JSCalendar types; `calcard` and `jiff` below are used through it.
* `calcard =0.3.14` (Apache-2.0 OR MIT): iCalendar <-> JSCalendar. **Pinned exactly**: later Stalwart releases are AGPL (do not use).
  It already emits the 2.0 shape (`recurrenceRule`, `organizerCalendarAddress`, no `timeZones`); our `upgrade` runs over its output anyway.
  I did not add the `icalendar` crate (MIT/Apache-2.0): it models iCalendar only and would need our own JSCalendar mapper, which calcard provides.
* `rrule 0.14.0` (MIT OR Apache-2.0): RFC 5545 recurrence engine. It is built on `chrono-tz` (tzdb **2025b**, no newer release exists: 0.10.4 is the latest), so we do not let it
  touch zones: the rule runs on **wall-clock time labelled UTC** (`DTSTART:...Z`, `UNTIL` as local wall-clock), and each resulting local start is converted with our zone type. `chrono-tz` is still
  compiled in as `rrule`'s dependency, but its data is never consulted.
* `jiff 0.2` (Unlicense OR MIT) + `jiff-tzdb` (same licence): zones. System tzdb preferred when it is at least as new as the bundled copy (feature `tzdb-bundle-always` keeps the fallback on Linux);
  `time::tzdb_version()` / `tzdb_source()` tell which one is used. Zone ids are validated (plain `Area/Location` names only).
* `serde`, `serde_json`, `chrono` (instants and arithmetic), `thiserror`; dev: `tokio`, `proptest`. Transitive licences are permissive (MIT/Apache/BSD/ISC/Unicode/Unlicense).

## Decisions

* **Time handling.** `start` is a local date-time in `timeZone`; no `timeZone` = floating, interpreted in the repo's floating zone (default UTC, `with_floating_zone`).
  DST: an ambiguous local time takes the first occurrence, a nonexistent one uses the offset before the gap. `P1D` is a nominal day (25 h across the autumn change), `PT1H` is exact.
  `Instance` windows are half-open `[from, to)`; zero-length events count when `from <= start < to`.
* **Overrides.** Keys are local date-times in the master's zone. `{"excluded": true}` removes an instance; other patches use JSCalendar paths
  (`title`, `participants/p1/participationStatus`, `start`, `null` removes); a key the rule does not produce is an extra instance (RDATE-like).
* **Object id = uid** when the `uid` is a `urn:uuid:` UUIDv7. Re-imports skip known `uid`s (originals are immutable).
* **Tasks** (envelope, RFC 9253 / 10051, queries, VTODO) are decided and documented in `fs-tasks` (ADR 0050). The calendar's `agenda` asks `TaskItems::query` for the tasks due in the window.
* **ICS originals.** One file with several UIDs is split textually: header + all `VTIMEZONE`s + that UID's components + `END:VCALENDAR`, lines copied unchanged
  (so the original per object is exact text of the source, though not the whole file). A one-UID file is kept whole.
* **Reserved-in-2.0 properties** (`sendTo`, `replyTo`, `localizations`, iTIP scheduling properties) are **kept in place**, not moved to legacy: the demo data uses `sendTo`,
  and they are needed for invitations. Revisit when a scheduling extension exists.
* **Export fixups.** Participants get `calendarAddress` from `email` / `sendTo.imip`; participants with no address at all (e.g. the demo's "Project owner") are omitted from the
  ICS because calcard cannot express them (it even `debug_assert`s). The converter call is wrapped in `catch_unwind`.

## Log mapping and fs-store 0.2

ADR 0020 maps invite -> `Invite`, replies -> `Accept` / `TentativeAccept` / `Reject`; fs-store 0.2 has all four kinds, so the 0.1 payload workaround (`Offer`/`Accept` with `payload.type`) is gone.
`invitation_log` still understands 0.1 entries. `respond` and `invite` use `Store::put_and_log`.

Still wanted in `fs-store`:

1. AS2 `location` as an envelope field (only the `startTime` / `endTime` pair exists).
2. Overlap queries are `start < to AND end >= from`; a series with a far-apart first start and last end is a candidate for every window in between. That is correct (it is expanded and filtered) but not selective for rarely-recurring series.
3. Per-actor free/busy needs `participant_email` + time range together; works today (AND), but without an aggregate.

## Open points

* **Question to the owner (provisional):** the shape of JSCalendar JSON on `exportText` for several events (JSON array now; a JSCalendar `Group` is the alternative). Reversible: the importer reads both.
* **`updateIf` and `Event.revision`** (`events.capnp` in fs-interfaces v0.35.0) are served: `Event.revision` is `Revision::of_text(updated, content)`, a 64-bit FNV-1a fingerprint, equal on every read until the event is written; `updateIf` calls `Calendar::update_checked`, a stale revision is `calendar/conflict` (409) with a detail starting `conflict:`, nothing is written. Atomic inside one server process only (the writes take turns), as the contract says.
* **`Items.occurrences` has no place for what was left out** (contract gap, known to the strategy): a cut is answered `too-large`; damage (an override or event that does not read) is only said on stderr by the server. The contract could return a list of `Problem`s next to the instances.
* **Writing many overrides is slow:** the check of the overrides (`validate/overrides.rs`) copies the master once per override, so 2 MiB with 20 000 overrides takes about 3 s under the write lock. Not changed.
* **Import is quadratic** (measured: ICS 1 000 / 2 000 / 4 000 events take 3 / 11 / 45 s, JSON 5 / 22 / 98 s; 20 000 take more than ten minutes under the write lock, probably `get_by_uid` for each event): a follow-up point, not changed.
* **Memory of writing** (measured by the review, not bounded here): a 15 MB text costs about 165 MiB to write, four at once about 660 MiB; an import of 2 MB with 15 000 events 127 MiB.
* **Module heads and a text comparison:** `model/counter.rs` says `none` because it is a mutable counter that also checks the limits (no pattern of coding.md section 2 describes that); `model/bounded.rs` recognises its own refusal by the text of the serde error (`split_once(MARK)`), which coding.md section 2a does not allow and should become a typed error.
* **Fail open for older data:** an event whose rule could not be written today is named alone when its window is too long (`cut: false`), not a `too-large` for everyone; an edit that leaves the rule alone (a new title) is not refused for it; `migrate_time_fields` does not check the rule (it can still stop at `TooHeavy`).
* **Small differences, not changed:** the depth is counted one level apart by `parse_bounded` and `build` (both answer 413); monthly and yearly rules are never shifted, so a dense allowed monthly rule is named after about three years; the pages of the store are read by `uid_key` after the last key, so a write between two pages can show an event twice or skip it; events stored before `uid_key` existed are not paged (no productive data).
* **Old data above the limits:** an event stored before the limits (more than 40 MiB estimated) is read as before, but `update` of it answers `TooHeavy`; `migrate_time_fields` stops at the first such event (to change: skip and report).
* **`count` beyond 65 535 of a sub-daily rule cannot be written** (its end cannot be computed in one range): `Error::Limit` at write, which used to be a plain recurrence error.
* **Unknown `TZID` is refused, not stored with a note** (provisional, easy to reverse): storing the event at the wrong time was the fault; a note beside a wrong time is still a wrong time.
* **`privacy` alone does not reach the envelope**: only the marking `freesynergy.net:sensitive` does. `CLASS:PRIVATE` events must stay storable in an unencrypted store. The frontend still treats `private` / `secret` as marked for its own export.
* **Module heads** use the names of coding.md section 2 as it stands since 2026-11-07 (Specification, Value object, Visitor, Constants are in it now); a test file names what it tests.
* **`normalise_uid` exists here and in `fs-people-core`** (same rule): it belongs in a shared crate (coding.md section 4), for example `fs-jscalendar`.
* **`Registry` / `Subscription` exist here and in `fs-tasks-core` and `fs-people-core`**: a generic one belongs in `fs-interfaces::rpc` or `fs-store`.
* The server is started without a task program: tasks in an imported file are noted, not stored. A client to `fs-tasks-server` would attach one.
* A start must be in years 1 to 9998 (`limits::FIRST_YEAR`, `LAST_YEAR`): the time library saturates at the edges of years 0 and 9999, so an event there would silently move.
* A series is expanded at most `MAX_INSTANCES_PER_EVENT` instances per question; the rule is walked at most `MAX_RULE_STEPS` steps from its start. A cheaper way would shift `DTSTART` by whole periods for daily and weekly rules (not done: `COUNT` and the `BY` rules depend on the real start).
* `agenda`, `upcoming`, `migrate_time_fields` and `get_event` are not in the contract; they stay on the trait as local helpers and use the nearest capability id.
* tzdb is picked once per process (first use); a system tzdata update needs a restart. Stored originals keep the version they were imported with, nothing is re-evaluated against a newer one.
* Objects written before fs-store 0.2 need `migrate_time_fields()` once (the store migration cannot compute them).
* Wall-clock recurrence: a start in a DST gap is not skipped (RFC 5545 allows either) but moved by the offset before the gap, as for single events.
* Per-instance invitation replies (an `Accept` for one occurrence only) are not modelled: `respond` changes the master's participant.
* Recurrence with `rscale` other than gregorian is refused with an error. `firstDayOfWeek`, `skip` are passed through partly (`WKST` yes, `skip` no).
* Alerts, `locations`, `virtualLocations`, `links` are preserved but not typed or interpreted.
* No scheduling/iTIP transport (`METHOD:REQUEST` etc.), no CalDAV; free/busy computation is not implemented.
* The ICS export is as good as calcard's; vendor properties become `JSPROP` lines (RFC 9554 style) and are not understood by other clients.
