# Calendar — Requirements

What `FreeSynergy.Calendar` must and can do, derived from the [frontends](frontends/index.md)
(ADR 0006, 0013). Each capability names its data standard (ADR 0014) and our status.

Status: `implemented` · `planned` · `proposed`

## Required (common to the analysed programs)

| Capability | Seen in | Data / standard | Status |
|------------|---------|-----------------|--------|

## Optional (notable ideas)

| Capability | Seen in | Data / standard | Status |
|------------|---------|-----------------|--------|
| Create an event or task from one typed sentence (date, time, place, calendar, alert, repetition) | Fantastical | JSCalendar `Event`/`Task` fields | proposed |
| Show several time zones side by side, labelled; temporary switch of the primary zone | Notion Calendar | IANA zones, JSCalendar `timeZone` | proposed |
| Share free slots as a link (one-off or recurring) with booking window and expiry | Notion Calendar, Cal.com | gap (free/busy, JSCalendar) | proposed |
| Public booking page per event type with availability, buffers, minimum notice | Cal.com | gap, shared specification | proposed |
| Place tasks into free calendar time by priority, deadline and duration, and re-plan on change | Reclaim, Motion | JSCalendar `Task` (RFC 9253), `Duration` | proposed |
| Repeating flexible blocks (habits, focus time) with frequency, duration and time window | Reclaim | JSCalendar `RecurrenceRule` | proposed |

## Not planned (deliberately)
