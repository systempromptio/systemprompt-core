//! [`EscalationDelivery`]: where this host delivers an escalation.
//!
//! Every host seeds one value — `InProcess` on the API,
//! `Consumer` on a child host — and the notifications member assembles its
//! service with it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

/// Where an escalated notification is delivered.
///
/// `InProcess` delivers it through the notifications pipeline at once.
/// `Consumer` leaves the row `pending` for the process that runs the
/// notifications consumer with the full extension seams; its sweep delivers
/// it. A process that holds only the core channels uses `Consumer`, so no
/// extension channel is skipped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EscalationDelivery {
    InProcess,
    Consumer,
}
