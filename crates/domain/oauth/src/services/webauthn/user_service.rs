use anyhow::Result;
use std::sync::Arc;
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
    ) -> Result<String> {
        if let Some(existing_user) = self
            .user_provider
            .find_by_email(email)
            .await
            .map_err(|e| anyhow::anyhow!("{}", e))?
        {
            return Ok(existing_user.id);
        }

        let roles = roles.unwrap_or_else(|| vec!["user".to_string()]);

        let user = self
            .user_provider
            .create_user(username, email, full_name)
            .await
            .map_err(|e| anyhow::anyhow!("{}", e))?;

        self.user_provider
            .assign_roles(&user.id, &roles)
            .await
            .map_err(|e| anyhow::anyhow!("{}", e))?;

        Ok(user.id)
    }

    pub async fn create_user_with_webauthn_registration(
        &self,
        username: &str,
        email: &str,
        full_name: Option<&str>,
    ) -> Result<String> {
        if self
            .user_provider
            .find_by_email(email)
            .await
            .map_err(|e| anyhow::anyhow!("{}", e))?
            .is_some()
        {
            return Err(anyhow::anyhow!("email_already_registered"));
        }

        if self
            .user_provider
            .find_by_name(username)
            .await
            .map_err(|e| anyhow::anyhow!("{}", e))?
            .is_some()
        {
            return Err(anyhow::anyhow!("username_already_taken"));
        }

        self.find_or_create_user_with_webauthn_registration(username, email, full_name, None)
            .await
    }
}
