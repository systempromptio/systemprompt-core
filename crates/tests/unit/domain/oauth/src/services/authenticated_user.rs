//! Tests for `load_authenticated_user`: the user is read through the
//! `UserProvider` trait, unparseable roles are skipped, a user left with no
//! permission is refused, and a missing user is an error.

use async_trait::async_trait;
use systemprompt_identifiers::UserId;
use systemprompt_oauth::services::load_authenticated_user;
use systemprompt_traits::{
    AuthProviderError, AuthResult, AuthUser, FederatedIdentityClaims, UserProvider,
};
use uuid::Uuid;

struct FixedUser(Option<AuthUser>);

#[async_trait]
impl UserProvider for FixedUser {
    async fn find_by_id(&self, _id: &UserId) -> AuthResult<Option<AuthUser>> {
        Ok(self.0.clone())
    }

    async fn find_by_email(&self, _email: &str) -> AuthResult<Option<AuthUser>> {
        Ok(None)
    }

    async fn find_by_name(&self, _name: &str) -> AuthResult<Option<AuthUser>> {
        Ok(None)
    }

    async fn create_user(
        &self,
        _name: &str,
        _email: &str,
        _full_name: Option<&str>,
    ) -> AuthResult<AuthUser> {
        Err(AuthProviderError::Internal("unused".into()))
    }

    async fn create_anonymous(&self, _fingerprint: &str) -> AuthResult<AuthUser> {
        Err(AuthProviderError::Internal("unused".into()))
    }

    async fn assign_roles(&self, _user_id: &UserId, _roles: &[String]) -> AuthResult<()> {
        Ok(())
    }

    async fn find_or_create_federated(
        &self,
        _issuer: &str,
        _external_sub: &str,
        _claims: &FederatedIdentityClaims,
    ) -> AuthResult<UserId> {
        Err(AuthProviderError::Internal("unused".into()))
    }

    async fn promote_anonymous(&self, _source: &UserId, _target: &UserId) -> AuthResult<u64> {
        Ok(0)
    }
}

struct FailingUsers;

#[async_trait]
impl UserProvider for FailingUsers {
    async fn find_by_id(&self, _id: &UserId) -> AuthResult<Option<AuthUser>> {
        Err(AuthProviderError::Internal("database unavailable".into()))
    }

    async fn find_by_email(&self, _email: &str) -> AuthResult<Option<AuthUser>> {
        Ok(None)
    }

    async fn find_by_name(&self, _name: &str) -> AuthResult<Option<AuthUser>> {
        Ok(None)
    }

    async fn create_user(
        &self,
        _name: &str,
        _email: &str,
        _full_name: Option<&str>,
    ) -> AuthResult<AuthUser> {
        Err(AuthProviderError::Internal("unused".into()))
    }

    async fn create_anonymous(&self, _fingerprint: &str) -> AuthResult<AuthUser> {
        Err(AuthProviderError::Internal("unused".into()))
    }

    async fn assign_roles(&self, _user_id: &UserId, _roles: &[String]) -> AuthResult<()> {
        Ok(())
    }

    async fn find_or_create_federated(
        &self,
        _issuer: &str,
        _external_sub: &str,
        _claims: &FederatedIdentityClaims,
    ) -> AuthResult<UserId> {
        Err(AuthProviderError::Internal("unused".into()))
    }

    async fn promote_anonymous(&self, _source: &UserId, _target: &UserId) -> AuthResult<u64> {
        Ok(0)
    }
}

fn user_with_roles(id: &UserId, roles: &[&str]) -> AuthUser {
    AuthUser {
        id: id.clone(),
        name: "alice".to_owned(),
        email: format!("{}@ou.invalid", id.as_str()),
        roles: roles.iter().map(|r| (*r).to_owned()).collect(),
        is_active: true,
    }
}

fn uuid_user_id() -> UserId {
    UserId::new(Uuid::new_v4().to_string())
}

#[tokio::test]
async fn resolves_permissions_and_keeps_roles() {
    let user_id = uuid_user_id();
    let users = FixedUser(Some(user_with_roles(&user_id, &["user"])));

    let authed = load_authenticated_user(&users, &user_id)
        .await
        .expect("authenticated user");

    assert_eq!(authed.email, format!("{}@ou.invalid", user_id.as_str()));
    assert_eq!(authed.username, "alice");
    assert!(!authed.permissions().is_empty());
    assert_eq!(authed.roles, vec!["user".to_owned()]);
}

#[tokio::test]
async fn skips_unparseable_roles() {
    let user_id = uuid_user_id();
    let users = FixedUser(Some(user_with_roles(
        &user_id,
        &["user", "not-a-real-role"],
    )));

    let authed = load_authenticated_user(&users, &user_id)
        .await
        .expect("valid role survives the bogus one");

    assert_eq!(authed.permissions().len(), 1);
    assert_eq!(authed.roles.len(), 2);
}

#[tokio::test]
async fn rejects_user_with_only_invalid_roles() {
    let user_id = uuid_user_id();
    let users = FixedUser(Some(user_with_roles(&user_id, &["bogus-role"])));

    let err = load_authenticated_user(&users, &user_id)
        .await
        .expect_err("no valid permissions must be rejected");

    assert!(err.to_string().contains("no valid permissions"));
}

#[tokio::test]
async fn rejects_non_uuid_user_id() {
    let user_id = UserId::new("not-a-uuid");
    let users = FixedUser(Some(user_with_roles(&user_id, &["user"])));

    let err = load_authenticated_user(&users, &user_id)
        .await
        .expect_err("a non-UUID id cannot become a principal");

    assert!(err.to_string().contains("Invalid user UUID"));
}

#[tokio::test]
async fn missing_user_errors() {
    let user_id = uuid_user_id();
    let users = FixedUser(None);

    let err = load_authenticated_user(&users, &user_id)
        .await
        .expect_err("missing user");

    assert!(err.to_string().contains("user not found"));
}

#[tokio::test]
async fn provider_failure_propagates() {
    let user_id = uuid_user_id();

    let err = load_authenticated_user(&FailingUsers, &user_id)
        .await
        .expect_err("provider failure is not a missing user");

    assert!(err.to_string().contains("database unavailable"));
}
