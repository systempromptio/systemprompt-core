use std::str::FromStr;

use async_trait::async_trait;
use systemprompt_identifiers::UserId;
use systemprompt_traits::auth::{
    AuthProviderError, AuthResult, AuthUser, FederatedIdentityClaims, RoleProvider, UserProvider,
};

use super::UserService;
use crate::models::{User, UserRole};

impl From<User> for AuthUser {
    fn from(user: User) -> Self {
        let is_active = user.is_active();
        Self {
            id: user.id,
            name: user.name,
            email: user.email,
            roles: user.roles,
            is_active,
        }
    }
}

#[async_trait]
impl UserProvider for UserService {
    async fn find_by_id(&self, id: &UserId) -> AuthResult<Option<AuthUser>> {
        self.find_by_id(id)
            .await
            .map(|opt| opt.map(AuthUser::from))
            .map_err(|e| AuthProviderError::Internal(e.to_string()))
    }

    async fn find_by_email(&self, email: &str) -> AuthResult<Option<AuthUser>> {
        Self::find_by_email(self, email)
            .await
            .map(|opt| opt.map(AuthUser::from))
            .map_err(|e| AuthProviderError::Internal(e.to_string()))
    }

    async fn find_by_name(&self, name: &str) -> AuthResult<Option<AuthUser>> {
        Self::find_by_name(self, name)
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
        Self::create(self, name, email, full_name, full_name)
            .await
            .map(AuthUser::from)
            .map_err(|e| AuthProviderError::Internal(e.to_string()))
    }

    async fn create_anonymous(&self, fingerprint: &str) -> AuthResult<AuthUser> {
        Self::create_anonymous(self, fingerprint)
            .await
            .map(AuthUser::from)
            .map_err(|e| AuthProviderError::Internal(e.to_string()))
    }

    async fn assign_roles(&self, user_id: &UserId, roles: &[String]) -> AuthResult<()> {
        Self::assign_roles(self, user_id, roles)
            .await
            .map(|_| ())
            .map_err(|e| AuthProviderError::Internal(e.to_string()))
    }

    async fn find_or_create_federated(
        &self,
        issuer: &str,
        external_sub: &str,
        claims: &FederatedIdentityClaims,
    ) -> AuthResult<UserId> {
        Self::find_or_create_federated(self, issuer, external_sub, claims)
            .await
            .map(|u| u.id)
            .map_err(|e| AuthProviderError::Internal(e.to_string()))
    }
}

#[async_trait]
impl RoleProvider for UserService {
    async fn get_roles(&self, user_id: &UserId) -> AuthResult<Vec<String>> {
        match Self::find_by_id(self, user_id).await {
            Ok(Some(user)) => Ok(user.roles),
            Ok(None) => Err(AuthProviderError::UserNotFound),
            Err(e) => Err(AuthProviderError::Internal(e.to_string())),
        }
    }

    async fn assign_role(&self, user_id: &UserId, role: &str) -> AuthResult<()> {
        let user = match Self::find_by_id(self, user_id).await {
            Ok(Some(u)) => u,
            Ok(None) => return Err(AuthProviderError::UserNotFound),
            Err(e) => return Err(AuthProviderError::Internal(e.to_string())),
        };

        let mut roles = user.roles;
        let role_str = role.to_owned();
        if !roles.contains(&role_str) {
            roles.push(role_str);
        }

        Self::assign_roles(self, user_id, &roles)
            .await
            .map(|_| ())
            .map_err(|e| AuthProviderError::Internal(e.to_string()))
    }

    async fn revoke_role(&self, user_id: &UserId, role: &str) -> AuthResult<()> {
        let user = match Self::find_by_id(self, user_id).await {
            Ok(Some(u)) => u,
            Ok(None) => return Err(AuthProviderError::UserNotFound),
            Err(e) => return Err(AuthProviderError::Internal(e.to_string())),
        };

        let roles: Vec<String> = user.roles.into_iter().filter(|r| r != role).collect();

        Self::assign_roles(self, user_id, &roles)
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
            .map(|users| users.into_iter().map(AuthUser::from).collect())
            .map_err(|e| AuthProviderError::Internal(e.to_string()))
    }
}
