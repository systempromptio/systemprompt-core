//! [`OwnerReassignment`](systemprompt_traits::OwnerReassignment) for the
//! events tables: moves every `event_outbox` row from one user to another.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

systemprompt_database::owner_reassignment! {
    pub struct EventsOwnerReassignment;
    domain = "events";
    update "event_outbox" (event_outbox.user_id) => "UPDATE event_outbox SET user_id = $2 WHERE user_id = $1";
}
