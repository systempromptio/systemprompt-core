//! The display name a grant records for its subject.
//!
//! A `user` subject shows the user's display name, falling back to the full
//! name and then the account name; an unknown user and every other subject
//! kind show the subject value itself.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::UserId;
use systemprompt_users::{UserError, UserRepository};

pub const USER_SUBJECT_KIND: &str = "user";

pub async fn subject_display(
    users: &UserRepository,
    subject_kind: &str,
    subject_value: &str,
) -> Result<String, UserError> {
    if subject_kind != USER_SUBJECT_KIND {
        return Ok(subject_value.to_owned());
    }
    let user = users.find_by_id(&UserId::new(subject_value)).await?;
    Ok(user.map_or_else(
        || subject_value.to_owned(),
        |user| user.display_name.or(user.full_name).unwrap_or(user.name),
    ))
}
