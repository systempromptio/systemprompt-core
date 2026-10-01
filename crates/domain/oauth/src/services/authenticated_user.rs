//! Resolves a user id into the `AuthenticatedUser` a token is minted for.
//!
//! The user row belongs to the users domain and is read through the shared
//! `UserProvider` trait. A user whose stored roles parse to no permission at
//! all is refused rather than handed a permission-less principal.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::str::FromStr;

use systemprompt_identifiers::UserId;
use systemprompt_models::auth::{AuthenticatedUser, Permission};
use systemprompt_traits::UserProvider;

use crate::error::{OauthError, OauthResult};

pub async fn load_authenticated_user(
    users: &dyn UserProvider,
    user_id: &UserId,
) -> OauthResult<AuthenticatedUser> {
    let user = users
        .find_by_id(user_id)
        .await
        .map_err(|source| OauthError::UserProvider {
            context: "loading the user a token is minted for",
            source,
        })?
        .ok_or_else(|| OauthError::UserNotFound(user_id.to_string()))?;

    let permissions: Vec<Permission> = user
        .roles
        .iter()
        .filter_map(|role| {
            Permission::from_str(role)
                .map_err(|e| {
                    tracing::warn!(
                        user_id = %user.id,
                        role = %role,
                        error = %e,
                        "Invalid role in user record"
                    );
                    e
                })
                .ok()
        })
        .collect();

    if permissions.is_empty() {
        return Err(OauthError::Validation(
            "User has no valid permissions after parsing".to_owned(),
        ));
    }

    user.id
        .to_uuid()
        .map_err(|_e| OauthError::Validation(format!("Invalid user UUID: {}", user.id)))?;

    Ok(AuthenticatedUser::new_with_roles(
        user.id,
        user.name,
        user.email,
        permissions,
        user.roles,
    ))
}
