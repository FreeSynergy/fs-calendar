# fs-calendar – Design

`fs-calendar` is the calendar program of FreeSynergy. This repo holds the **program core** (`crates/fs-calendar-core`, no GUI).
Events are **JSCalendar 2.0** objects (`draft-ietf-calext-jscalendarbis-22`) inside AS2 envelopes stored through
`fs-store` (ADR 0014, 0019, 0020). Every program is at least a workbench (ADR 0037); GUI crates come later and use this core.

## State

Version 0.4.0: `Calendar` follows the provider rule (ADR 0045): every method of `events.capnp`, defaults answering `Unsupported`, `capabilities()` returns `Capabilities`, and `import_text` / `export_text` by media type. Built on fs-store 0.2 (git tag) and [`fs-jscalendar`](https://github.com/FreeSynergy/fs-jscalendar) v0.1.0, which holds what `Event` and `Task` share (time, rule, participants, upgrade, shared validation, iCalendar conversion). `Event` itself stays here (ADR 0050).

**Task moved out (ADR 0050).** `Task` is owned by `fs-tasks`; this repo has no Task model, validation, upgrade, queries or VTODO code any more.
The calendar depends on the **`TaskItems` trait** of `fs-tasks-core` (git tag `v0.4.0`) and shows tasks only through it
(`CalendarRepo::with_tasks`, `agenda`). `CalendarObject` (the Event-or-Task enum) is gone: the API takes `Event`.

Depends on `fs-tasks-core` v0.4.0 and `fs-interfaces` v0.4.0 (git tags).

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
| Provider rule: `Error::Unsupported`; required are `capabilities()` and the reads (`get`, `get_event`, `get_by_uid`, `list_events`, `occurrences`), everything else defaults to `Unsupported`; `CalendarRepo` reports all ids of the schema except `event.subscribe` ("not built yet; it is possible") | done |
| `importText` / `exportText` by media type (`TextFormat`, Factory): `text/calendar` and `application/jscalendar+json` (parameters and case ignored); any other type is `Error::MediaType`, nothing is stored | done |
| JSCalendar JSON export (`JsCalendarFormat` is now also an `Exporter`): one event = the object, several = a JSON array; both read back through the importer | done, provisional (see open points) |
| Tests | 55 now: `tests/provider.rs` (2) and `tests/media.rs` (3) are new on top of the 50 below (3 new in `tests/task_forwarding.rs`; before them: the 43 event tests are unchanged; the 2 task tests and the task parts of 2 others moved to fs-tasks, 4 new in `tests/tasks.rs` cover the task interface: fake `TaskItems`, real `TaskRepo` over the same store, `.ics` with `VTODO`) (before the move: 45; 39 pre-refactor tests unchanged in behaviour, now split by feature into `tests/{model,crud,queries,tasks,invitations,import,ics,recur,recur_overrides,time}.rs`; the 6 former unit tests live in `tests/time.rs`; 6 new in `tests/interfaces.rs` and `tests/time.rs` cover the trait seams) |
| ADR 0043 refactor | done: every file <= 200 lines (`check_rust_rules.py`: 0 errors, 0 warnings), `//! Pattern:` in every module, `#![deny(missing_docs)]`, `#![warn(clippy::pedantic)]` clean |

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
| `repo/{crud,update,queries,migrate}.rs` | Repository / Template method | writes, sequence-aware update with `put_and_log`, range queries, idempotent migration |
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

* `fs-store` (git tag v0.2.0). `fs-interfaces` (git tag v0.4.0): `Capabilities`, `Unsupported`. `fs-tasks-core` (git tag v0.4.0): the `TaskItems` trait and the task types it uses. `fs-jscalendar` (git tag v0.1.0): the shared JSCalendar types; `calcard` and `jiff` below are used through it.
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
* **`event.subscribe`:** in the contract, not built. `supportReport` is `capabilities()`; the capnp-rpc server is not written.
* `agenda`, `upcoming`, `migrate_time_fields` and `get_event` are not in the contract; they stay on the trait as local helpers and use the nearest capability id.
* tzdb is picked once per process (first use); a system tzdata update needs a restart. Stored originals keep the version they were imported with, nothing is re-evaluated against a newer one.
* Objects written before fs-store 0.2 need `migrate_time_fields()` once (the store migration cannot compute them).
* Wall-clock recurrence: a start in a DST gap is not skipped (RFC 5545 allows either) but moved by the offset before the gap, as for single events.
* Per-instance invitation replies (an `Accept` for one occurrence only) are not modelled: `respond` changes the master's participant.
* Recurrence with `rscale` other than gregorian is refused with an error. `firstDayOfWeek`, `skip` are passed through partly (`WKST` yes, `skip` no).
* `rrule` takes a `u16` limit per call: a window with more than 65 535 instances of one event (secondly rules) is truncated.
* Alerts, `locations`, `virtualLocations`, `links` are preserved but not typed or interpreted.
* No scheduling/iTIP transport (`METHOD:REQUEST` etc.), no CalDAV; free/busy computation is not implemented.
* The ICS export is as good as calcard's; vendor properties become `JSPROP` lines (RFC 9554 style) and are not understood by other clients.
