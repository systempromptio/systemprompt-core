use axum::http::{HeaderMap, HeaderValue};
use std::sync::Arc;
use systemprompt_agent::models::a2a::jsonrpc::NumberOrString;
use systemprompt_agent::services::a2a_server::auth::validate_oauth_for_request;
use systemprompt_models::auth::Permission;
use systemprompt_traits::{
    AgentJwtClaims, GenerateTokenParams, JwtProviderError, JwtResult, JwtValidationProvider,
};

struct StubJwtProvider {
    claims: Option<AgentJwtClaims>,
}

impl StubJwtProvider {
    fn ok(claims: AgentJwtClaims) -> Arc<dyn JwtValidationProvider> {
        Arc::new(Self {
            claims: Some(claims),
        })
    }

    fn err() -> Arc<dyn JwtValidationProvider> {
        Arc::new(Self { claims: None })
    }
}

impl JwtValidationProvider for StubJwtProvider {
    fn validate_token(&self, _token: &str) -> JwtResult<AgentJwtClaims> {
        if let Some(c) = self.claims.clone() {
            Ok(c)
        } else {
            Err(JwtProviderError::InvalidToken)
        }
    }
    fn generate_token(&self, _params: GenerateTokenParams) -> JwtResult<String> {
        Ok("token".to_string())
    }
    fn generate_secure_token(&self, prefix: &str) -> String {
        format!("{prefix}-fake")
    }
}

fn bearer(token: &str) -> HeaderMap {
    let mut h = HeaderMap::new();
    h.insert(
        "authorization",
        HeaderValue::from_str(&format!("Bearer {token}")).unwrap(),
    );
    h
}

fn claims_admin() -> AgentJwtClaims {
    AgentJwtClaims {
        subject: "user-1".to_string(),
        username: "admin".to_string(),
        user_type: "admin".to_string(),
        audiences: vec!["a2a".to_string()],
        permissions: vec!["admin".to_string()],
        is_admin: true,
        expires_at: 9_999_999_999,
        issued_at: 0,
    }
}

fn claims_user(perms: &[&str]) -> AgentJwtClaims {
    AgentJwtClaims {
        subject: "user-2".to_string(),
        username: "alice".to_string(),
        user_type: "user".to_string(),
        audiences: vec!["a2a".to_string()],
        permissions: perms.iter().map(|s| (*s).to_string()).collect(),
        is_admin: false,
        expires_at: 9_999_999_999,
        issued_at: 0,
    }
}

#[tokio::test]
async fn validate_oauth_no_bearer_returns_unauthorized() {
    let id = NumberOrString::Number(1);
    let provider = StubJwtProvider::ok(claims_admin());
    let result =
        validate_oauth_for_request(&HeaderMap::new(), &id, &[Permission::User], Some(&provider))
            .await;
    let err = result.expect_err("should be unauthorized");
    assert_eq!(err.0, axum::http::StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn validate_oauth_empty_token_returns_unauthorized() {
    let mut headers = HeaderMap::new();
    headers.insert("authorization", HeaderValue::from_static("Bearer "));
    let id = NumberOrString::Number(1);
    let provider = StubJwtProvider::ok(claims_admin());
    let err = validate_oauth_for_request(&headers, &id, &[], Some(&provider))
        .await
        .expect_err("unauthorized");
    assert_eq!(err.0, axum::http::StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn validate_oauth_no_provider_returns_unauthorized() {
    let headers = bearer("token-xyz");
    let id = NumberOrString::Number(1);
    let err = validate_oauth_for_request(&headers, &id, &[], None)
        .await
        .expect_err("no provider");
    assert_eq!(err.0, axum::http::StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn validate_oauth_invalid_token_returns_unauthorized() {
    let headers = bearer("bad");
    let id = NumberOrString::Number(1);
    let provider = StubJwtProvider::err();
    let err = validate_oauth_for_request(&headers, &id, &[], Some(&provider))
        .await
        .expect_err("invalid");
    assert_eq!(err.0, axum::http::StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn validate_oauth_admin_bypasses_scope_check() {
    let headers = bearer("token");
    let id = NumberOrString::Number(7);
    let provider = StubJwtProvider::ok(claims_admin());
    let result =
        validate_oauth_for_request(&headers, &id, &[Permission::User], Some(&provider)).await;
    let value = result.expect("ok").expect("Some");
    assert_eq!(value.get("is_admin"), Some(&serde_json::json!(true)));
    assert_eq!(value.get("username"), Some(&serde_json::json!("admin")));
}

#[tokio::test]
async fn validate_oauth_missing_audience_forbidden() {
    let headers = bearer("token");
    let mut claims = claims_admin();
    claims.audiences = vec!["other".to_string()];
    claims.is_admin = false; // forbidden path
    let provider = StubJwtProvider::ok(claims);
    let id = NumberOrString::Number(1);
    let err = validate_oauth_for_request(&headers, &id, &[], Some(&provider))
        .await
        .expect_err("forbidden");
    assert_eq!(err.0, axum::http::StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn validate_oauth_user_with_matching_permission_succeeds() {
    let headers = bearer("token");
    let id = NumberOrString::Number(1);
    let provider = StubJwtProvider::ok(claims_user(&["admin"]));
    let result =
        validate_oauth_for_request(&headers, &id, &[Permission::User], Some(&provider)).await;
    let value = result.expect("ok").expect("Some");
    assert_eq!(value.get("username"), Some(&serde_json::json!("alice")));
}

#[tokio::test]
async fn validate_oauth_user_lacking_permission_forbidden() {
    let headers = bearer("token");
    let id = NumberOrString::Number(1);
    let provider = StubJwtProvider::ok(claims_user(&[]));
    let err = validate_oauth_for_request(&headers, &id, &[Permission::Admin], Some(&provider))
        .await
        .expect_err("forbidden");
    assert_eq!(err.0, axum::http::StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn validate_oauth_string_id_compatible() {
    let headers = bearer("token");
    let id = NumberOrString::String("req-abc".to_string());
    let provider = StubJwtProvider::ok(claims_admin());
    let result = validate_oauth_for_request(&headers, &id, &[], Some(&provider)).await;
    let value = result.expect("ok").expect("Some");
    assert_eq!(value.get("username"), Some(&serde_json::json!("admin")));
}
