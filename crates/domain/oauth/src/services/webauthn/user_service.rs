//! User creation/lookup wrapper used by `WebAuthn` flows.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::OauthResult as Result;
use std::sync::Arc;
use systemprompt_identifiers::UserId;
use systemprompt_traits::UserProvider;

pub struct UserCreationService {
    user_provider: Arc<dyn UserProvider>,
}

impl std::fmt::Debug for UserCreationService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UserCreationService").finish()
    }
}

impl UserCreationService {
    pub fn new(user_provider: Arc<dyn UserProvider>) -> Self {
        Self { user_provider }
    }

    pub async fn find_or_create_user_with_webauthn_registration(
        &self,
        username: &str,
        email: &str,
        full_name: Option<&str>,
        roles: Option<Vec<String>>,
    ) -> Result<UserId> {
        if let Some(existing_user) =
            self.user_provider
                .find_by_email(email)
                .await
                .map_err(|source| crate::error::OauthError::UserProvider {
                    context: "looking up the registering email",
                    source,
                })?
        {
            return Ok(existing_user.id);
        }

        let roles = roles.unwrap_or_else(|| vec!["user".to_owned()]);

        let user = self
            .user_provider
            .create_user(username, email, full_name)
            .await
            .map_err(|source| crate::error::OauthError::UserProvider {
                context: "creating the user",
                source,
            })?;

        self.user_provider
            .assign_roles(&user.id, &roles)
            .await
            .map_err(|source| crate::error::OauthError::UserProvider {
                context: "assigning the new user's roles",
                source,
            })?;

        Ok(user.id)
    }

    pub async fn create_user_with_webauthn_registration(
        &self,
        username: &str,
        email: &str,
        full_name: Option<&str>,
    ) -> Result<UserId> {
        if self
            .user_provider
            .find_by_email(email)
            .await
            .map_err(|source| crate::error::OauthError::UserProvider {
                context: "checking whether the email is registered",
                source,
            })?
            .is_some()
        {
            return Err(crate::error::OauthError::EmailRegistered(email.to_owned()));
        }

        if self
            .user_provider
            .find_by_name(username)
            .await
            .map_err(|source| crate::error::OauthError::UserProvider {
                context: "checking whether the username is taken",
                source,
            })?
            .is_some()
        {
            return Err(crate::error::OauthError::UsernameTaken(username.to_owned()));
        }

        self.find_or_create_user_with_webauthn_registration(username, email, full_name, None)
            .await
    }
}
