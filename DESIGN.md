# fs-calendar – Design

`fs-calendar` is the calendar program of FreeSynergy. This repo holds the **program core** (`crates/fs-calendar-core`, no GUI).
Events and tasks are **JSCalendar 2.0** objects (`draft-ietf-calext-jscalendarbis-22`) inside AS2 envelopes stored through
`fs-store` (ADR 0014, 0019, 0020). Every program is at least a workbench (ADR 0037); GUI crates come later and use this core.

## State

Version 0.1.0, local only (no remote, never pushed).

| Area | State |
|---|---|
| Typed `Event` and `Task` (serde), lossless unknown/vendor properties (`extra` on every typed object) | done; the 5 demo events round-trip equal as JSON |
| Validation (`version` "2.0", `uid`, `start`, LocalDateTime without zone suffix/fraction, IANA zones only, durations, status/privacy/freeBusy/progress enums, single `recurrenceRule`, count XOR until, override keys, obsolete 1.0 properties rejected) | done |
| `upgrade()` RFC 8984 -> 2.0: `recurrenceRules[0]` -> `recurrenceRule` (rest to `freesynergy.net:legacy`, with a note), obsolete properties moved to `freesynergy.net:legacy`, `Z` suffix -> `Etc/UTC`, fractions stripped, custom zones moved out; idempotent; Groups | done |
| Repository `CalendarRepo` over `Arc<dyn Store>`: create / get / update / delete / list | done |
| Envelope: type `Event` (events) / `Task` (tasks), `name` = `title` (ADR 0032, placeholder if untitled), `context=calendar`, IRI `https://<instance>/events/<id>` | done |
| Index fields: `uid`, `type`, `start_utc`, `end_utc`, `due_utc`, `recurring`, `status`, `progress`, `sequence`, `participant_email`, `calendar_id`, `tzdb_version` (imports) | done |
| Time-range query `occurrences(from, to)` / `upcoming(now, days)` / `tasks_due` | done; candidates narrowed by `start_utc`/`end_utc` **in Rust** (store has exact-match index only) |
| Recurrence expansion at query time: FREQ DAILY/WEEKLY/MONTHLY/YEARLY (and the rest `rrule` supports), INTERVAL, COUNT, UNTIL, BYDAY (incl. nth), BYMONTHDAY, BYMONTH, BYSETPOS, ...; overrides: excluded, patched (PatchObject paths), extra instances; wall-clock time kept across DST | done |
| Series end for the index (`end_utc` of the last instance, `9999-12-31T23:59:59Z` if unbounded) | done |
| Invitations as log activities: `invite` (`Invite`), `respond` (`Accept` / `TentativeAccept` / `Reject`) update `participationStatus` and log; `invitation_log` | done, with the store-kind mapping below |
| Sequence handling: `sequence` + 1 when start/zone/duration/rule/status (task: start/due/zone) change | done |
| iCalendar import (VEVENT/VTODO incl. RRULE, EXDATE, attendees, alarms) via `calcard`; original kept per UID (`text/calendar`), tzdb version recorded | done |
| iCalendar export | done (participants without any address are left out; see below) |
| Example `cargo run -p fs-calendar-core --example demo`: import demo events, next 30 days, accept an invitation, export | done |
| Tests | 31: unit (durations, DST, zones), integration over a real in-memory `TursoStore`, 3 property tests for recurrence (count/interval spacing in local days across zones, window consistency, inclusive UNTIL) |

## Crates

* `fs-store` (path dependency, not modified).
* `calcard =0.3.14` (Apache-2.0 OR MIT): iCalendar <-> JSCalendar. **Pinned exactly**: later Stalwart releases are AGPL (do not use).
  It already emits the 2.0 shape (`recurrenceRule`, `organizerCalendarAddress`, no `timeZones`); our `upgrade` runs over its output anyway.
  I did not add the `icalendar` crate (MIT/Apache-2.0): it models iCalendar only and would need our own JSCalendar mapper, which calcard provides.
* `rrule 0.14.0` (MIT OR Apache-2.0): RFC 5545 recurrence engine on `chrono-tz`. We translate the JSCalendar rule to `RRULE`, apply overrides ourselves.
* `chrono-tz 0.10.4` (MIT OR Apache-2.0): IANA zones, pure Rust. **tzdb inside is 2025b**, the docs plan 2026f; `time::TZDB_VERSION` exposes it and imports record it.
  `jiff` (Unlicense OR MIT) was looked at; `rrule` is built on chrono-tz, so using both would mean two zone databases.
* `serde`, `serde_json`, `chrono`, `thiserror`; dev: `tokio`, `proptest`. Transitive licences are permissive (MIT/Apache/BSD/ISC/Unicode/Unlicense).

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

## Log mapping and what `fs-store` needs (not changed here)

ADR 0020 maps invite -> `Invite`, replies -> `Accept` / `TentativeAccept` / `Reject`. The store's `ActivityKind` has neither `Invite` nor `TentativeAccept`:

* `Invite` is written as kind `Offer` (AS2: `Invite` is a subtype of `Offer`) with payload `{"type":"Invite", ...}`.
* `TentativeAccept` is written as kind `Accept` (subtype) with payload `{"type":"TentativeAccept", ...}`.
* `invitation_log` reads the payload `type`, so callers see the real names.

Wanted in `fs-store`:

1. `ActivityKind::Invite` and `TentativeAccept` (log-only kinds) so payload sniffing is unnecessary.
2. **Atomic object change + log entry in one call** (e.g. `put_and_log`): `respond` does `put` (writes `Update`) and then `log` (writes `Accept`), two transactions; a crash between them leaves the status changed without reply entry.
3. **Range / ordered index queries** (`start_utc < x AND end_utc > y`); today every `occurrences()` call loads all Events and filters in Rust. Fine for personal calendars, not for shared ones with ten thousands of events.
4. Envelope fields for AS2 `startTime` / `endTime` / `location` (technical.md lists them); only index fields exist now.
5. Case-insensitive name ordering (not needed here yet, same as in fs-contacts).

## Open points

* tzdb 2025b vs planned 2026f: needs a newer `chrono-tz` or a bundled tzdb loader; no way to hot-update yet.
* Per-instance invitation replies (an `Accept` for one occurrence only) are not modelled: `respond` changes the master's participant.
* Recurrence with `rscale` other than gregorian is refused with an error. `firstDayOfWeek`, `skip` are passed through partly (`WKST` yes, `skip` no).
* `rrule` takes a `u16` limit per call: a window with more than 65 535 instances of one event (secondly rules) is truncated.
* Alerts, `locations`, `virtualLocations`, `links` are preserved but not typed or interpreted.
* No scheduling/iTIP transport (`METHOD:REQUEST` etc.), no CalDAV; free/busy computation is not implemented.
* The ICS export is as good as calcard's; vendor properties become `JSPROP` lines (RFC 9554 style) and are not understood by other clients.
