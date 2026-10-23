//! [`OwnerReassignment`](systemprompt_traits::OwnerReassignment) for the files
//! domain: moves every file row from one user to another.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

systemprompt_database::owner_reassignment! {
    pub struct FilesOwnerReassignment;
    domain = "files";
    update "files" => "UPDATE files SET user_id = $2 WHERE user_id = $1";
}
