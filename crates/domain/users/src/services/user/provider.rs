use std::str::FromStr;

use async_trait::async_trait;
use systemprompt_identifiers::UserId;
use systemprompt_traits::auth::{
    AuthProviderError, AuthResult, AuthUser, RoleProvider, UserProvider,
};

use super::UserService;
use crate::models::{User, UserRole};

#[async_trait]
impl UserProvider for UserService {
    async fn find_by_id(&self, id: &str) -> AuthResult<Option<AuthUser>> {
        let user_id = UserId::new(id);
        self.find_by_id(&user_id)
            .await
            .map(|opt| opt.map(|u| user_to_auth_user(&u)))
            .map_err(|e| AuthProviderError::Internal(e.to_string()))
    }

    async fn find_by_email(&self, email: &str) -> AuthResult<Option<AuthUser>> {
        Self::find_by_email(self, email)
            .await
            .map(|opt| opt.map(|u| user_to_auth_user(&u)))
            .map_err(|e| AuthProviderError::Internal(e.to_string()))
    }

    async fn find_by_name(&self, name: &str) -> AuthResult<Option<AuthUser>> {
        Self::find_by_name(self, name)
            .await
            .map(|opt| opt.map(|u| user_to_auth_user(&u)))
            .map_err(|e| AuthProviderError::Internal(e.to_string()))
    }

    async fn create_user(
        &self,
        name: &str,
        email: &str,
        full_name: Option<&str>,
    ) -> AuthResult<AuthUser> {
        Self::create(self, name, email, full_name, full_name)
            .await
            .map(|u| user_to_auth_user(&u))
            .map_err(|e| AuthProviderError::Internal(e.to_string()))
    }

    async fn create_anonymous(&self, fingerprint: &str) -> AuthResult<AuthUser> {
        Self::create_anonymous(self, fingerprint)
            .await
            .map(|u| user_to_auth_user(&u))
            .map_err(|e| AuthProviderError::Internal(e.to_string()))
    }

    async fn assign_roles(&self, user_id: &str, roles: &[String]) -> AuthResult<()> {
        let id = UserId::new(user_id);
        Self::assign_roles(self, &id, roles)
            .await
            .map(|_| ())
            .map_err(|e| AuthProviderError::Internal(e.to_string()))
    }
}

fn user_to_auth_user(user: &User) -> AuthUser {
    AuthUser {
        id: user.id.to_string(),
        name: user.name.clone(),
        email: user.email.clone(),
        roles: user.roles.clone(),
        is_active: user.is_active(),
    }
}

#[async_trait]
impl RoleProvider for UserService {
    async fn get_roles(&self, user_id: &str) -> AuthResult<Vec<String>> {
        let id = UserId::new(user_id);
        match Self::find_by_id(self, &id).await {
            Ok(Some(user)) => Ok(user.roles),
            Ok(None) => Err(AuthProviderError::UserNotFound),
            Err(e) => Err(AuthProviderError::Internal(e.to_string())),
        }
    }

    async fn assign_role(&self, user_id: &str, role: &str) -> AuthResult<()> {
        let id = UserId::new(user_id);
        let user = match Self::find_by_id(self, &id).await {
            Ok(Some(u)) => u,
            Ok(None) => return Err(AuthProviderError::UserNotFound),
            Err(e) => return Err(AuthProviderError::Internal(e.to_string())),
        };

        let mut roles = user.roles;
        let role_str = role.to_string();
        if !roles.contains(&role_str) {
            roles.push(role_str);
        }

        Self::assign_roles(self, &id, &roles)
            .await
            .map(|_| ())
            .map_err(|e| AuthProviderError::Internal(e.to_string()))
    }

    async fn revoke_role(&self, user_id: &str, role: &str) -> AuthResult<()> {
        let id = UserId::new(user_id);
        let user = match Self::find_by_id(self, &id).await {
            Ok(Some(u)) => u,
            Ok(None) => return Err(AuthProviderError::UserNotFound),
            Err(e) => return Err(AuthProviderError::Internal(e.to_string())),
        };

        let roles: Vec<String> = user.roles.into_iter().filter(|r| r != role).collect();

        Self::assign_roles(self, &id, &roles)
            .await
            .map(|_| ())
            .map_err(|e| AuthProviderError::Internal(e.to_string()))
    }

    async fn list_users_by_role(&self, role: &str) -> AuthResult<Vec<AuthUser>> {
        let Ok(user_role) = UserRole::from_str(role) else {
            return Ok(vec![]);
        };

        Self::find_by_role(self, user_role)
            .await
            .map(|users| users.iter().map(user_to_auth_user).collect())
            .map_err(|e| AuthProviderError::Internal(e.to_string()))
    }
}
