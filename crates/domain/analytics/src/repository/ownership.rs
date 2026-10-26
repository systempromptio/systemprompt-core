//! [`OwnerReassignment`](systemprompt_traits::OwnerReassignment) for the
//! analytics domain: moves every engagement event from one user to another and
//! rewrites the source id in each fingerprint's associated-user list, in a
//! single transaction.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

systemprompt_database::owner_reassignment! {
    pub struct AnalyticsOwnerReassignment;
    domain = "analytics";
    update "engagement_events" => "UPDATE engagement_events SET user_id = $2 WHERE user_id = $1";
    update "fingerprint_reputation" => "UPDATE fingerprint_reputation SET associated_user_ids = \
             array_replace(associated_user_ids, $1, $2) WHERE $1 = ANY(associated_user_ids)";
}
