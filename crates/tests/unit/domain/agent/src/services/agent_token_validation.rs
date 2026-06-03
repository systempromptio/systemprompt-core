// Tests for validate_agent_token and AgentSessionUser::from_jwt_claims.
//
// AgentOAuthState is built with a DB pool but NO user_provider, so the
// validate path exercises: missing-provider error, invalid-token error,
// audience enforcement, and the success branch that maps claims into an
// AgentSessionUser.

use std::sync::Arc;
use systemprompt_agent::services::a2a_server::auth::{
    AgentOAuthConfig, AgentOAuthState, validate_agent_token,
};
use systemprompt_agent::services::shared::auth::AgentSessionUser;
use systemprompt_models::auth::JwtAudience;
use systemprompt_traits::{
    AgentJwtClaims, GenerateTokenParams, JwtProviderError, JwtResult, JwtValidationProvider,
};

use crate::repository::try_pool;

struct StubJwtProvider {
    claims: Option<AgentJwtClaims>,
}

impl JwtValidationProvider for StubJwtProvider {
    fn validate_token(&self, _token: &str) -> JwtResult<AgentJwtClaims> {
        self.claims.clone().ok_or(JwtProviderError::InvalidToken)
    }
    fn generate_token(&self, _params: GenerateTokenParams) -> JwtResult<String> {
        Ok("token".to_string())
    }
    fn generate_secure_token(&self, prefix: &str) -> String {
        format!("{prefix}-fake")
    }
}

fn claims(audiences: Vec<&str>) -> AgentJwtClaims {
    AgentJwtClaims {
        subject: "user-aaa".to_string(),
        username: "alice".to_string(),
        user_type: "user".to_string(),
        audiences: audiences.into_iter().map(str::to_owned).collect(),
        permissions: vec!["user".to_string()],
        is_admin: false,
        expires_at: 9_999_999_999,
        issued_at: 0,
    }
}

fn state_with_provider(
    pool: &systemprompt_database::DbPool,
    provider: Option<AgentJwtClaims>,
) -> AgentOAuthState {
    let st = AgentOAuthState::new(
        Arc::clone(pool),
        AgentOAuthConfig::default(),
        "test-issuer".to_string(),
        vec![JwtAudience::A2a],
    );
    st.with_jwt_provider(Arc::new(StubJwtProvider { claims: provider }))
}

#[tokio::test]
async fn validate_agent_token_success_maps_session_user() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let state = state_with_provider(&pool, Some(claims(vec!["a2a"])));
    let user = validate_agent_token("tok", &state)
        .await
        .expect("valid token");
    assert_eq!(user.id.as_str(), "user-aaa");
    assert_eq!(user.username, "alice");
    assert_eq!(user.user_type, "user");
    assert!(user.permissions.contains(&"user".to_string()));
}

#[tokio::test]
async fn validate_agent_token_wrong_audience_errors() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let state = state_with_provider(&pool, Some(claims(vec!["web"])));
    let err = validate_agent_token("tok", &state)
        .await
        .expect_err("wrong audience");
    assert!(format!("{err}").contains("A2A"));
}

#[tokio::test]
async fn validate_agent_token_invalid_token_errors() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let state = state_with_provider(&pool, None);
    let err = validate_agent_token("tok", &state)
        .await
        .expect_err("invalid token");
    assert!(format!("{err}").to_lowercase().contains("invalid"));
}

#[tokio::test]
async fn validate_agent_token_no_provider_errors() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let state = AgentOAuthState::new(
        Arc::clone(&pool),
        AgentOAuthConfig::default(),
        "iss".to_string(),
        vec![JwtAudience::A2a],
    );
    let err = validate_agent_token("tok", &state)
        .await
        .expect_err("no provider");
    assert!(format!("{err}").contains("JWT provider"));
}

#[test]
fn agent_session_user_from_jwt_claims_maps_fields() {
    let user = AgentSessionUser::from_jwt_claims(AgentJwtClaims {
        subject: "sub-1".to_string(),
        username: "bob".to_string(),
        user_type: "admin".to_string(),
        audiences: vec!["a2a".to_string()],
        permissions: vec!["admin".to_string(), "user".to_string()],
        is_admin: true,
        expires_at: 1,
        issued_at: 0,
    });
    assert_eq!(user.id.as_str(), "sub-1");
    assert_eq!(user.username, "bob");
    assert_eq!(user.user_type, "admin");
    assert_eq!(user.permissions, vec!["admin".to_string(), "user".to_string()]);
}
