//! The participation state machine of an invitation.
//!
//! Pattern: State — [`ParticipationStatus`] is the state, [`Response`] the event, and
//! [`ParticipationStatus::respond`] the one transition function. A participant starts as
//! `needs-action` and may answer (again) at any time, so every answer leads to its own state.

use fs_store::ActivityKind;

/// `participationStatus` of a JSCalendar participant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParticipationStatus {
    /// Invited, no answer yet.
    NeedsAction,
    /// Accepted.
    Accepted,
    /// Declined.
    Declined,
    /// Tentatively accepted.
    Tentative,
    /// Delegated to somebody else.
    Delegated,
}

/// An answer to an invitation (ADR 0020 log mapping).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Response {
    /// Accept the invitation.
    Accept,
    /// Tentatively accept.
    TentativeAccept,
    /// Decline.
    Reject,
}

impl ParticipationStatus {
    /// The JSCalendar names of all states.
    pub const NAMES: [&'static str; 5] = fs_jscalendar::model::PARTICIPATION_STATUSES;

    /// State of a freshly invited participant.
    pub const INITIAL: Self = Self::NeedsAction;

    /// Read a JSCalendar `participationStatus`.
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "needs-action" => Self::NeedsAction,
            "accepted" => Self::Accepted,
            "declined" => Self::Declined,
            "tentative" => Self::Tentative,
            "delegated" => Self::Delegated,
            _ => return None,
        })
    }

    /// The JSCalendar name of this state.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NeedsAction => "needs-action",
            Self::Accepted => "accepted",
            Self::Declined => "declined",
            Self::Tentative => "tentative",
            Self::Delegated => "delegated",
        }
    }

    /// The transition: the state after answering with `response`. Any state may answer again.
    #[must_use]
    pub fn respond(self, response: Response) -> Self {
        match response {
            Response::Accept => Self::Accepted,
            Response::TentativeAccept => Self::Tentative,
            Response::Reject => Self::Declined,
        }
    }
}

impl Response {
    /// AS2 activity type name.
    pub fn as_str(&self) -> &'static str {
        match self {
            Response::Accept => "Accept",
            Response::TentativeAccept => "TentativeAccept",
            Response::Reject => "Reject",
        }
    }

    /// JSCalendar `participationStatus` this answer leads to.
    pub fn status(&self) -> &'static str {
        ParticipationStatus::INITIAL.respond(*self).as_str()
    }

    /// The activity kind written to the log.
    pub(crate) fn kind(self) -> ActivityKind {
        match self {
            Response::Accept => ActivityKind::Accept,
            Response::TentativeAccept => ActivityKind::TentativeAccept,
            Response::Reject => ActivityKind::Reject,
        }
    }
}
