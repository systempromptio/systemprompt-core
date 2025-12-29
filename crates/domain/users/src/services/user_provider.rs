use async_trait::async_trait;
use systemprompt_identifiers::UserId;
use systemprompt_traits::{AuthProviderError, AuthResult, AuthUser, UserProvider};

use crate::UserService;

#[derive(Debug, Clone)]
pub struct UserProviderImpl {
    user_service: UserService,
}

impl UserProviderImpl {
    pub const fn new(user_service: UserService) -> Self {
        Self { user_service }
    }
}

impl From<crate::User> for AuthUser {
    fn from(user: crate::User) -> Self {
        Self {
            id: user.id.to_string(),
            name: user.name,
            email: user.email,
            roles: user.roles,
            is_active: user.status.as_deref() == Some("active"),
        }
    }
}

#[async_trait]
impl UserProvider for UserProviderImpl {
    async fn find_by_id(&self, id: &str) -> AuthResult<Option<AuthUser>> {
        let user_id = UserId::new(id.to_string());
        self.user_service
            .find_by_id(&user_id)
            .await
            .map(|opt| opt.map(AuthUser::from))
            .map_err(|e| AuthProviderError::Internal(e.to_string()))
    }

    async fn find_by_email(&self, email: &str) -> AuthResult<Option<AuthUser>> {
        self.user_service
            .find_by_email(email)
            .await
            .map(|opt| opt.map(AuthUser::from))
            .map_err(|e| AuthProviderError::Internal(e.to_string()))
    }

    async fn find_by_name(&self, name: &str) -> AuthResult<Option<AuthUser>> {
        self.user_service
            .find_by_name(name)
            .await
            .map(|opt| opt.map(AuthUser::from))
            .map_err(|e| AuthProviderError::Internal(e.to_string()))
    }

    async fn create_user(
        &self,
        name: &str,
        email: &str,
        full_name: Option<&str>,
    ) -> AuthResult<AuthUser> {
        self.user_service
            .create(name, email, full_name, None)
            .await
            .map(AuthUser::from)
            .map_err(|e| AuthProviderError::Internal(e.to_string()))
    }

    async fn create_anonymous(&self, fingerprint: &str) -> AuthResult<AuthUser> {
        self.user_service
            .create_anonymous(fingerprint)
            .await
            .map(AuthUser::from)
            .map_err(|e| AuthProviderError::Internal(e.to_string()))
    }

    async fn assign_roles(&self, user_id: &str, roles: &[String]) -> AuthResult<()> {
        let id = UserId::new(user_id.to_string());
        self.user_service
            .assign_roles(&id, roles)
            .await
            .map(|_| ())
            .map_err(|e| AuthProviderError::Internal(e.to_string()))
    }
}
