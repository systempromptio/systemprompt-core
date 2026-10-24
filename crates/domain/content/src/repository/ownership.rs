//! [`OwnerReassignment`](systemprompt_traits::OwnerReassignment) for the
//! content domain: moves every recorded link click from one user to another.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

systemprompt_database::owner_reassignment! {
    pub struct ContentOwnerReassignment;
    domain = "content";
    update "link_clicks" => "UPDATE link_clicks SET user_id = $2 WHERE user_id = $1";
}
