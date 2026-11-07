//! What this client offers of what the server reports.
//!
//! Pattern: Adapter (a filter on the report) — provider rule, ADR 0045: `capabilities()` never
//! promises more than the methods of `RpcCalendar` keep. The report is cut to what the client
//! implements; the rest is reported as not possible, with the reason.

use fs_interfaces::Capabilities;

/// What the server can do and this client does not call: reported as not possible.
pub(crate) const NOT_OFFERED: [&str; 4] = [
    "invite.create",
    "invite.answer",
    "invite.read",
    "event.subscribe",
];

/// The report of the server, cut to what this client implements.
pub(crate) fn offered(reported: &Capabilities) -> Capabilities {
    let mut caps = Capabilities::new(reported.provider());
    for entry in reported.entries() {
        if entry.supported && NOT_OFFERED.contains(&entry.capability.as_str()) {
            caps.unsupported(
                entry.capability.as_str(),
                "the server offers it, the client of the calendar program does not yet",
            );
        } else if entry.supported {
            caps.supported(entry.capability.as_str());
        } else {
            caps.unsupported(entry.capability.as_str(), entry.reason.as_str());
        }
    }
    caps
}
