//! [`OwnerReassignment`](systemprompt_traits::OwnerReassignment) for the
//! logging tables: moves every log line and analytics event from one user to
//! another in a single transaction.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

systemprompt_database::owner_reassignment! {
    pub struct LoggingOwnerReassignment;
    domain = "logging";
    update "logs" (logs.user_id) => "UPDATE logs SET user_id = $2 WHERE user_id = $1";
    update "analytics_events" (analytics_events.user_id) => "UPDATE analytics_events SET user_id = $2 WHERE user_id = $1";
}
