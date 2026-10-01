//! User identifier — an opaque, checked string.
//!
//! Every `UserId` names a row in the `users` table. New users are minted with
//! a UUIDv4 string ([`UserId::generate`]), but the column is TEXT and existing
//! deployments hold non-UUID ids (seeded admins, imported users, service
//! accounts), so the shape is not part of the contract. `try_new` rejects
//! only what can never be a user id: an empty or whitespace-bearing value,
//! control characters, and the retired `"unset"` sentinel. It is the
//! constructor for values arriving from outside (a JWT `sub`, a header, a
//! path segment); `new` is for values already known to be valid, such as a
//! decoded `users.id`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::IdValidationError;

crate::define_id!(UserId, checked, validate_user_id);

fn validate_user_id(value: &str) -> Result<(), IdValidationError> {
    if value.is_empty() {
        return Err(IdValidationError::empty("UserId"));
    }
    if value.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(IdValidationError::invalid(
            "UserId",
            "must not contain whitespace or control characters",
        ));
    }
    if value.eq_ignore_ascii_case("unset") {
        return Err(IdValidationError::invalid(
            "UserId",
            "'unset' is a retired sentinel, not a user id",
        ));
    }
    Ok(())
}

impl UserId {
    pub fn generate() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }
}
