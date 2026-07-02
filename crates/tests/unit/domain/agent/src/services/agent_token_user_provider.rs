// validate_agent_token with a user_provider installed: exercises the
// user-lookup and permission-verification branches (unknown user, inactive
// user, no valid permissions, non-admin denial, and the admin success paths
// via token claims and DB roles).

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_agent::services::a2a_server::auth::{
    AgentOAuthConfig, AgentOAuthState, validate_agent_token,
};
use systemprompt_identifiers::UserId;
use systemprompt_models::auth::JwtAudience;
use systemprompt_traits::{
    AgentJwtClaims, AuthProviderError, AuthResult, AuthUser, FederatedIdentityClaims,
    GenerateTokenParams, JwtResult, JwtValidationProvider, UserProvider,
};

use crate::repository::try_pool;

struct StubJwtProvider {
    claims: AgentJwtClaims,
}

impl JwtValidationProvider for StubJwtProvider {
    fn validate_token(&self, _token: &str) -> JwtResult<AgentJwtClaims> {
        Ok(self.claims.clone())
    }
    fn generate_token(&self, _params: GenerateTokenParams) -> JwtResult<String> {
        Ok("token".to_owned())
    }
    fn generate_secure_token(&self, prefix: &str) -> String {
        format!("{prefix}-fake")
    }
}

struct StubUserProvider {
    user: Option<AuthUser>,
    fail: bool,
}

#[async_trait]
impl UserProvider for StubUserProvider {
    async fn find_by_id(&self, _id: &UserId) -> AuthResult<Option<AuthUser>> {
        if self.fail {
            return Err(AuthProviderError::Internal("lookup failed".to_owned()));
        }
        Ok(self.user.clone())
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
        Err(AuthProviderError::InvalidCredentials)
    }
    async fn create_anonymous(&self, _fingerprint: &str) -> AuthResult<AuthUser> {
        Err(AuthProviderError::InvalidCredentials)
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
        Err(AuthProviderError::InvalidCredentials)
    }
}

fn claims(is_admin: bool, permissions: Vec<&str>) -> AgentJwtClaims {
    AgentJwtClaims {
        subject: "user-perm".to_owned(),
        username: "alice".to_owned(),
        user_type: "user".to_owned(),
        audiences: vec!["a2a".to_owned()],
        permissions: permissions.into_iter().map(str::to_owned).collect(),
        is_admin,
        expires_at: 9_999_999_999,
        issued_at: 0,
    }
}

fn user(roles: Vec<&str>, is_active: bool) -> AuthUser {
    AuthUser {
        id: UserId::new("user-perm"),
        name: "alice".to_owned(),
        email: "alice@example.invalid".to_owned(),
        roles: roles.into_iter().map(str::to_owned).collect(),
        is_active,
    }
}

fn state(
    pool: &systemprompt_database::DbPool,
    jwt_claims: AgentJwtClaims,
    provider: StubUserProvider,
) -> AgentOAuthState {
    AgentOAuthState::new(
        Arc::clone(pool),
        AgentOAuthConfig::default(),
        "test-issuer".to_owned(),
        vec![JwtAudience::A2a],
    )
    .with_jwt_provider(Arc::new(StubJwtProvider { claims: jwt_claims }))
    .with_user_provider(Arc::new(provider))
}

#[tokio::test]
async fn unknown_user_is_rejected() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let st = state(
        &pool,
        claims(true, vec!["admin"]),
        StubUserProvider {
            user: None,
            fail: false,
        },
    );
    let err = validate_agent_token("t", &st).await.expect_err("no user");
    assert!(err.to_string().contains("User not found"));
}

#[tokio::test]
async fn lookup_failure_is_internal_error() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let st = state(
        &pool,
        claims(true, vec!["admin"]),
        StubUserProvider {
            user: None,
            fail: true,
        },
    );
    let err = validate_agent_token("t", &st).await.expect_err("db error");
    assert!(err.to_string().contains("Failed to lookup user"));
}

#[tokio::test]
async fn inactive_user_is_rejected() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let st = state(
        &pool,
        claims(true, vec!["admin"]),
        StubUserProvider {
            user: Some(user(vec!["admin"], false)),
            fail: false,
        },
    );
    let err = validate_agent_token("t", &st).await.expect_err("inactive");
    assert!(err.to_string().contains("not active"));
}

#[tokio::test]
async fn user_without_valid_permissions_is_rejected() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let st = state(
        &pool,
        claims(true, vec!["admin"]),
        StubUserProvider {
            user: Some(user(vec!["not_a_permission"], true)),
            fail: false,
        },
    );
    let err = validate_agent_token("t", &st)
        .await
        .expect_err("no permissions");
    assert!(err.to_string().contains("no valid permissions"));
}

#[tokio::test]
async fn non_admin_token_and_non_admin_roles_are_rejected() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let st = state(
        &pool,
        claims(false, vec!["user"]),
        StubUserProvider {
            user: Some(user(vec!["user"], true)),
            fail: false,
        },
    );
    let err = validate_agent_token("t", &st).await.expect_err("denied");
    assert!(err.to_string().contains("lacks required A2A permissions"));
}

#[tokio::test]
async fn admin_token_with_active_user_succeeds() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let st = state(
        &pool,
        claims(true, vec!["admin"]),
        StubUserProvider {
            user: Some(user(vec!["user"], true)),
            fail: false,
        },
    );
    let session_user = validate_agent_token("t", &st).await.expect("admin token");
    assert_eq!(session_user.username, "alice");
}

#[tokio::test]
async fn admin_db_role_with_non_admin_token_succeeds() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let st = state(
        &pool,
        claims(false, vec!["user"]),
        StubUserProvider {
            user: Some(user(vec!["admin"], true)),
            fail: false,
        },
    );
    let session_user = validate_agent_token("t", &st).await.expect("db admin");
    assert_eq!(session_user.username, "alice");
}
