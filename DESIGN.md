# fs-calendar – Design

`fs-calendar` is the calendar program of FreeSynergy. This repo holds the **program core** (`crates/fs-calendar-core`, no GUI).
Events and tasks are **JSCalendar 2.0** objects (`draft-ietf-calext-jscalendarbis-22`) inside AS2 envelopes stored through
`fs-store` (ADR 0014, 0019, 0020). Every program is at least a workbench (ADR 0037); GUI crates come later and use this core.

## State

Version 0.1.0 (refactored to ADR 0043), local only (no remote, never pushed). Built on fs-store 0.2 (path dependency) and jiff for time zones.

| Area | State |
|---|---|
| Typed `Event` and `Task` (serde), lossless unknown/vendor properties (`extra` on every typed object) | done; the 5 demo events round-trip equal as JSON |
| Validation (`version` "2.0", `uid`, `start`, LocalDateTime without zone suffix/fraction, IANA zones only, durations, status/privacy/freeBusy/progress enums, single `recurrenceRule`, count XOR until, override keys, obsolete 1.0 properties rejected) | done |
| `upgrade()` RFC 8984 -> 2.0: `recurrenceRules[0]` -> `recurrenceRule` (rest to `freesynergy.net:legacy`, with a note), obsolete properties moved to `freesynergy.net:legacy`, `Z` suffix -> `Etc/UTC`, fractions stripped, custom zones moved out; idempotent; Groups | done |
| Contract `Calendar` (trait, object-safe) + store-backed `CalendarRepo` (create / get / update / delete / list / queries / invitations / import / export); depends on `dyn ObjectRepository` + `dyn ObjectQuery` + `dyn ActivityLog` only | done |
| Recurrence engine behind `RecurrenceEngine` (default `RRuleEngine` on `rrule`; `CalendarRepo::with_recurrence_engine`) | done |
| Formats behind `Importer` / `Exporter` (`IcsFormat`, `JsCalendarFormat`); `CalendarRepo::import(&dyn Importer, ..)` / `export(&dyn Exporter, ..)` | done |
| Invitation flow as State: `ParticipationStatus` + `Response`, transition `respond`; `invite`/`respond` write via `put_and_log` | done |
| Time-zone source as Strategy: `ZoneSource` (`SystemTzdb`, `BundledTzdb`), `time::newest` picks, `Zone::parse_in` for an explicit source | done |
| Envelope: type `Event` (events) / `Task` (tasks), `name` = `title` (ADR 0032, placeholder if untitled), `context=calendar`, IRI `https://<instance>/events/<id>`, **`startTime` / `endTime`** (first start; end of the last instance, `9999-12-31T23:59:59Z` if unbounded) | done |
| Index fields (`start_utc` / `end_utc` are kept next to the envelope times for compatibility and for `migrate_time_fields`): `uid`, `type`, `start_utc`, `end_utc`, `due_utc`, `recurring`, `status`, `progress`, `sequence`, `participant_email`, `calendar_id`, `tzdb_version` (imports) | done |
| Time-range query `occurrences(from, to)` / `upcoming(now, days)` / `tasks_due` | done; candidates selected **by the store** (`start_time < to AND end_time >= from` on the envelope), only those are expanded; `tasks_due` = store range on `due_utc`; `list_events` / `list_tasks` ordered by the store. `migrate_time_fields()` fills the envelope times of objects written before fs-store 0.2 |
| Recurrence expansion at query time: FREQ DAILY/WEEKLY/MONTHLY/YEARLY (and the rest `rrule` supports), INTERVAL, COUNT, UNTIL, BYDAY (incl. nth), BYMONTHDAY, BYMONTH, BYSETPOS, ...; overrides: excluded, patched (PatchObject paths), extra instances; wall-clock time kept across DST | done |
| Series end for the index (`end_utc` of the last instance, `9999-12-31T23:59:59Z` if unbounded) | done |
| Invitations as log activities: `invite` (`Invite`), `respond` (`Accept` / `TentativeAccept` / `Reject`) update `participationStatus` and log, **in one transaction** (`put_and_log`, fault-injection test); `invitation_log` (also reads 0.1 entries) | done, real AS2 kinds |
| Sequence handling: `sequence` + 1 when start/zone/duration/rule/status (task: start/due/zone) change | done |
| iCalendar import (VEVENT/VTODO incl. RRULE, EXDATE, attendees, alarms) via `calcard`; original kept per UID (`text/calendar`), tzdb version recorded | done |
| iCalendar export | done (participants without any address are left out; see below) |
| Example `cargo run -p fs-calendar-core --example demo`: import demo events, next 30 days, accept an invitation, export | done |
| Time zones | `jiff` with the newest tzdb available: system copy (`$TZDIR`, `/usr/share/zoneinfo`, version from `tzdata.zi`) if not older than the bundled `jiff-tzdb` copy, else the bundled one. Here: **tzdb 2026e** from `/usr/share/zoneinfo` (bundled copy is also 2026e). Version recorded as index `tzdb_version` with each imported original | done |
| Tests | 45 (39 pre-refactor tests unchanged in behaviour, now split by feature into `tests/{model,crud,queries,tasks,invitations,import,ics,recur,recur_overrides,time}.rs`; the 6 former unit tests live in `tests/time.rs`; 6 new in `tests/interfaces.rs` and `tests/time.rs` cover the trait seams) |
| ADR 0043 refactor | done: every file <= 200 lines (`check_rust_rules.py`: 0 errors, 0 warnings), `//! Pattern:` in every module, `#![deny(missing_docs)]`, `#![warn(clippy::pedantic)]` clean |

## Structure and pattern map (ADR 0043)

Public contract: the trait `Calendar`; `CalendarRepo` is its implementation (Repository + Facade) and keeps all its
inherent methods (generic `create` / `update` take `impl Into<CalendarObject>`), so callers that do not import the trait
keep working. Every file is one concept, <= 200 lines (tests included); `lib.rs` and every `mod.rs` only declare and re-export.

| Module | Pattern | Why |
|---|---|---|
| `calendar.rs` | Repository (interface) | the program's public contract; GUI, hub and tests use `dyn Calendar` |
| `repo/handle.rs` | Repository + Facade | one small type hides store, envelope rules, formats and recurrence |
| `repo/backend.rs` | Adapter | `Arc<dyn Store>` presented as `dyn ObjectRepository` + `dyn ObjectQuery` + `dyn ActivityLog` |
| `repo/{crud,update,queries,migrate}.rs` | Repository / Template method | writes, sequence-aware update with `put_and_log`, range queries, idempotent migration |
| `repo/{build,span}.rs` | Adapter / Strategy (by kind) | object to AS2 envelope + index fields; event span via the engine, task span from `due` |
| `repo/transfer.rs` | Template method | fixed import flow (split, skip uid, parse, normalise, store, report); split / parse are the `Importer` |
| `format/*`, `ical/*` | Adapter (+ Strategy) | `Importer` / `Exporter` traits; iCalendar (calcard) and JSCalendar JSON behind them |
| `invite/status.rs` | State | `ParticipationStatus` x `Response` -> next state (single transition function) |
| `invite/flow.rs` | State (client) | `respond` / `invite` apply the transition and log in one `put_and_log` transaction |
| `invite/log.rs` | Observer-style read model | invitation history from the activity log (reads 0.1 entries too) |
| `recur/engine.rs`, `recur/rrule_engine.rs` | Strategy + Adapter | `RecurrenceEngine` yields a rule's local starts; `RRuleEngine` wraps `rrule` on wall-clock time |
| `recur/expand.rs` | Template method | overrides, exclusions, patches, zone conversion and window filter around any engine |
| `recur/patch.rs` | Command | JSCalendar `PatchObject` applied to a base value |
| `time/source.rs`, `bundled.rs`, `system.rs` | Strategy + Template method | `ZoneSource`: where tz rules come from; shared id validation is a provided method |
| `time/select.rs` | Factory | picks the newest of system and bundled tzdb at first use |
| `time/{zone,dur,format}.rs` | Value objects | `Zone` (UTC / IANA), nominal-day + exact-second `Dur`, wire formats |
| `model/*` | Value objects / Template method (macro) | typed JSCalendar with lossless `extra`; macros fix the shared skeleton |
| `validate/*` | Specification | JSCalendar 2.0 rules reporting into one `Ctx`; all issues at once |
| `upgrade.rs` | Pipeline | RFC 8984 -> 2.0 rewrite steps, idempotent, with notes |
| `error.rs`, `clock.rs` | none | plain error enum, pure helper |

## Migration for dependents

* `CalendarRepo::store()` is removed: the repository holds only the narrow store traits. Keep your own `Arc<dyn Store>`
  for undo / log / count calls (the old tests did `r.store().undo(..)`).
* `repo::Response`, `InvitationEntry` moved to `invite` (still re-exported at the crate root).
* `time::TzDb` is replaced by the `ZoneSource` trait with `SystemTzdb` / `BundledTzdb`; `TzDb::detect/newest/bundled/from_dir`
  became `time::detect`, `time::newest`, `BundledTzdb::new`, `SystemTzdb::from_dir`. `time::tzdb_version()` / `tzdb_source()` are unchanged.
* `recur::{expand, series_end}` keep their signatures (default engine); new `expand_with` / `series_end_with` take a `&dyn RecurrenceEngine`.
  `recur::rrule_string` keeps its path.
* `CalendarRepo::new/with_instance/with_floating_zone/create/get/get_event/get_task/update/delete/list_*/occurrences/upcoming/
  tasks_due/invite/respond/invitation_log/import_*/export_ics`, `Event`, `Task`, `Zone`, `utc`, `validate::*`, `upgrade` keep their paths
  and signatures; fs-hub compiles unchanged.
* New: `Calendar`, `Importer`, `Exporter`, `RecurrenceEngine`, `ZoneSource`, `ParticipationStatus`,
  `CalendarRepo::{with_backend, with_recurrence_engine, import, export}`. Inherent methods win over trait methods, so importing
  `Calendar` next to `CalendarRepo` is not ambiguous.

## Crates

* `fs-store` (path dependency, not modified).
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
* **Task envelope type** is `Task` (AS2 has no task type); content type is the same JSCalendar media type. RFC 10051 status lives in `freesynergy.net:taskStatus` (preserved, not interpreted).
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

* tzdb is picked once per process (first use); a system tzdata update needs a restart. Stored originals keep the version they were imported with, nothing is re-evaluated against a newer one.
* Objects written before fs-store 0.2 need `migrate_time_fields()` once (the store migration cannot compute them).
* Wall-clock recurrence: a start in a DST gap is not skipped (RFC 5545 allows either) but moved by the offset before the gap, as for single events.
* Per-instance invitation replies (an `Accept` for one occurrence only) are not modelled: `respond` changes the master's participant.
* Recurrence with `rscale` other than gregorian is refused with an error. `firstDayOfWeek`, `skip` are passed through partly (`WKST` yes, `skip` no).
* `rrule` takes a `u16` limit per call: a window with more than 65 535 instances of one event (secondly rules) is truncated.
* Alerts, `locations`, `virtualLocations`, `links` are preserved but not typed or interpreted.
* No scheduling/iTIP transport (`METHOD:REQUEST` etc.), no CalDAV; free/busy computation is not implemented.
* The ICS export is as good as calcard's; vendor properties become `JSPROP` lines (RFC 9554 style) and are not understood by other clients.
