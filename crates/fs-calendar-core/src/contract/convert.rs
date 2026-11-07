//! Wire types <-> the Rust types of `Calendar`.
//!
//! Pattern: Adapter — small pure functions between the generated Cap'n Proto enum and
//! `Response`; the RPC server (`fs-calendar-server`) calls these.

use crate::events_capnp::Response as Wire;
use crate::invite::Response;

/// Map to the wire enum.
pub fn response_to_wire(r: Response) -> Wire {
    match r {
        Response::Accept => Wire::Accept,
        Response::TentativeAccept => Wire::TentativeAccept,
        Response::Reject => Wire::Reject,
    }
}

/// Map from the wire enum.
pub fn response_from_wire(w: Wire) -> Response {
    match w {
        Wire::Accept => Response::Accept,
        Wire::TentativeAccept => Response::TentativeAccept,
        Wire::Reject => Response::Reject,
    }
}
