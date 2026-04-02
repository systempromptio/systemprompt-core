use chrono::Utc;
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use systemprompt_identifiers::SessionId;
use systemprompt_models::auth::JwtAudience;
use systemprompt_oauth::{JwtAuthProvider, JwtAuthorizationProvider, JwtValidationProviderImpl, TraitBasedAuthService};
use systemprompt_traits::{AuthAction, AuthProvider, AuthorizationProvider, JwtValidationProvider};

const TEST_SECRET: &str = "test_secret_key_for_auth_provider_tests_12345";
const TEST_ISSUER: &str = "https://test.systemprompt.io";

#[derive(Debug, Serialize, Deserialize)]
struct TestClaims {
    sub: String,
    iat: i64,
    exp: i64,
    iss: String,
    aud: Vec<String>,
    jti: String,
    scope: String,
    username: String,
    email: String,
    user_type: String,
    roles: Vec<String>,
    token_type: String,
    auth_time: i64,
}

fn create_test_claims(exp_offset_secs: i64, issuer: &str, audiences: &[&str]) -> TestClaims {
    let now = Utc::now().timestamp();
    TestClaims {
        sub: "550e8400-e29b-41d4-a716-446655440000".to_string(),
        iat: now,
        exp: now + exp_offset_secs,
        iss: issuer.to_string(),
        aud: audiences.iter().map(|s| s.to_string()).collect(),
        jti: "test-jti-auth-provider".to_string(),
        scope: "user".to_string(),
        username: "testuser".to_string(),
        email: "test@example.com".to_string(),
        user_type: "user".to_string(),
        roles: vec!["user".to_string()],
        token_type: "Bearer".to_string(),
        auth_time: now,
    }
}

fn create_test_token(claims: &TestClaims, secret: &str) -> String {
    let header = Header::new(Algorithm::HS256);
    encode(&header, claims, &EncodingKey::from_secret(secret.as_bytes())).unwrap()
}

fn default_audiences() -> Vec<JwtAudience> {
    vec![JwtAudience::Api]
}

fn create_jwt_auth_provider() -> JwtAuthProvider {
    JwtAuthProvider::new(
        TEST_SECRET.to_string(),
        TEST_ISSUER.to_string(),
        default_audiences(),
    )
}

fn create_jwt_authz_provider() -> JwtAuthorizationProvider {
    JwtAuthorizationProvider::new(
        TEST_SECRET.to_string(),
        TEST_ISSUER.to_string(),
        default_audiences(),
    )
}

fn create_valid_token() -> String {
    let claims = create_test_claims(3600, TEST_ISSUER, &["api"]);
    create_test_token(&claims, TEST_SECRET)
}

// ============================================================================
// JwtAuthProvider Tests
// ============================================================================

#[test]
fn test_jwt_auth_provider_new() {
    let provider = create_jwt_auth_provider();
    let debug = format!("{provider:?}");
    assert!(debug.contains("JwtAuthProvider"));
}

#[tokio::test]
async fn test_jwt_auth_provider_validate_token_success() {
    let provider = create_jwt_auth_provider();
    let token = create_valid_token();

    let result = provider.validate_token(&token).await;

    let claims = result.expect("valid token should succeed");
    assert_eq!(claims.subject, "550e8400-e29b-41d4-a716-446655440000");
    assert_eq!(claims.username, "testuser");
    assert!(!claims.audiences.is_empty());
}

#[tokio::test]
async fn test_jwt_auth_provider_validate_token_extracts_email() {
    let provider = create_jwt_auth_provider();
    let token = create_valid_token();

    let claims = provider.validate_token(&token).await.expect("valid token should succeed");

    assert_eq!(claims.email, Some("test@example.com".to_string()));
}

#[tokio::test]
async fn test_jwt_auth_provider_validate_token_extracts_permissions() {
    let provider = create_jwt_auth_provider();
    let mut test_claims = create_test_claims(3600, TEST_ISSUER, &["api"]);
    test_claims.scope = "user admin".to_string();
    let token = create_test_token(&test_claims, TEST_SECRET);

    let claims = provider.validate_token(&token).await.expect("valid token should succeed");

    assert!(claims.permissions.len() >= 2);
    assert!(claims.permissions.iter().any(|p| p == "user"));
    assert!(claims.permissions.iter().any(|p| p == "admin"));
}

#[tokio::test]
async fn test_jwt_auth_provider_validate_token_expired() {
    let provider = create_jwt_auth_provider();
    let test_claims = create_test_claims(-3600, TEST_ISSUER, &["api"]);
    let token = create_test_token(&test_claims, TEST_SECRET);

    let result = provider.validate_token(&token).await;

    result.unwrap_err();
}

#[tokio::test]
async fn test_jwt_auth_provider_validate_token_wrong_secret() {
    let provider = create_jwt_auth_provider();
    let test_claims = create_test_claims(3600, TEST_ISSUER, &["api"]);
    let token = create_test_token(&test_claims, "wrong_secret_key");

    let result = provider.validate_token(&token).await;

    result.unwrap_err();
}

#[tokio::test]
async fn test_jwt_auth_provider_validate_token_malformed() {
    let provider = create_jwt_auth_provider();

    let result = provider.validate_token("not.a.valid.jwt").await;

    result.unwrap_err();
}

#[tokio::test]
async fn test_jwt_auth_provider_refresh_token_not_implemented() {
    let provider = create_jwt_auth_provider();

    let result = provider.refresh_token("some-refresh-token").await;

    let err = result.unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("not yet implemented"), "expected not-implemented error, got: {msg}");
}

#[tokio::test]
async fn test_jwt_auth_provider_revoke_token_not_implemented() {
    let provider = create_jwt_auth_provider();

    let result = provider.revoke_token("some-token").await;

    let err = result.unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("not yet implemented"), "expected not-implemented error, got: {msg}");
}

// ============================================================================
// JwtAuthorizationProvider Tests
// ============================================================================

#[tokio::test]
async fn test_jwt_authz_provider_authorize_always_true() {
    let provider = create_jwt_authz_provider();

    let result = provider
        .authorize("user-123", "some-resource", &AuthAction::Read)
        .await;

    assert!(result.expect("authorize should succeed"));
}

#[tokio::test]
async fn test_jwt_authz_provider_get_permissions_empty() {
    let provider = create_jwt_authz_provider();

    let result = provider.get_permissions("user-123").await;

    let permissions = result.expect("get_permissions should succeed");
    assert!(permissions.is_empty());
}

#[tokio::test]
async fn test_jwt_authz_provider_has_audience_matching() {
    let provider = create_jwt_authz_provider();
    let token = create_valid_token();

    let result = provider.has_audience(&token, "api").await;

    assert!(result.expect("has_audience should succeed"));
}

#[tokio::test]
async fn test_jwt_authz_provider_has_audience_not_matching() {
    let provider = create_jwt_authz_provider();
    let token = create_valid_token();

    let result = provider.has_audience(&token, "nonexistent").await;

    assert!(!result.expect("has_audience should succeed"));
}

#[tokio::test]
async fn test_jwt_authz_provider_has_audience_invalid_token() {
    let provider = create_jwt_authz_provider();

    let result = provider.has_audience("bad-token", "api").await;

    result.unwrap_err();
}

#[tokio::test]
async fn test_jwt_authz_provider_has_audience_multiple() {
    let provider = JwtAuthorizationProvider::new(
        TEST_SECRET.to_string(),
        TEST_ISSUER.to_string(),
        vec![JwtAudience::Api, JwtAudience::Web],
    );
    let test_claims = create_test_claims(3600, TEST_ISSUER, &["api", "web"]);
    let token = create_test_token(&test_claims, TEST_SECRET);

    let has_api = provider.has_audience(&token, "api").await.expect("should succeed for api");
    let has_web = provider.has_audience(&token, "web").await.expect("should succeed for web");
    let has_mcp = provider.has_audience(&token, "mcp").await.expect("should succeed for mcp check");

    assert!(has_api);
    assert!(has_web);
    assert!(!has_mcp);
}

#[tokio::test]
async fn test_jwt_authz_provider_has_audience_expired_token() {
    let provider = create_jwt_authz_provider();
    let test_claims = create_test_claims(-3600, TEST_ISSUER, &["api"]);
    let token = create_test_token(&test_claims, TEST_SECRET);

    let result = provider.has_audience(&token, "api").await;

    result.unwrap_err();
}

// ============================================================================
// TraitBasedAuthService Tests
// ============================================================================

#[test]
fn test_trait_based_auth_service_new() {
    let auth: Arc<dyn AuthProvider> = Arc::new(create_jwt_auth_provider());
    let authz: Arc<dyn AuthorizationProvider> = Arc::new(create_jwt_authz_provider());

    let service = TraitBasedAuthService::new(auth, authz);

    let debug = format!("{service:?}");
    assert!(debug.contains("TraitBasedAuthService"));
}

#[tokio::test]
async fn test_trait_based_auth_service_validate_token() {
    let auth: Arc<dyn AuthProvider> = Arc::new(create_jwt_auth_provider());
    let authz: Arc<dyn AuthorizationProvider> = Arc::new(create_jwt_authz_provider());
    let service = TraitBasedAuthService::new(auth, authz);
    let token = create_valid_token();

    let result = service.validate_token(&token).await;

    let claims = result.expect("validate_token should succeed");
    assert_eq!(claims.subject, "550e8400-e29b-41d4-a716-446655440000");
    assert_eq!(claims.username, "testuser");
}

#[tokio::test]
async fn test_trait_based_auth_service_has_audience() {
    let auth: Arc<dyn AuthProvider> = Arc::new(create_jwt_auth_provider());
    let authz: Arc<dyn AuthorizationProvider> = Arc::new(create_jwt_authz_provider());
    let service = TraitBasedAuthService::new(auth, authz);
    let token = create_valid_token();

    let has_api = service.has_audience(&token, "api").await.expect("should succeed");
    let has_mcp = service.has_audience(&token, "mcp").await.expect("should succeed");

    assert!(has_api);
    assert!(!has_mcp);
}

#[test]
fn test_trait_based_auth_service_debug() {
    let auth: Arc<dyn AuthProvider> = Arc::new(create_jwt_auth_provider());
    let authz: Arc<dyn AuthorizationProvider> = Arc::new(create_jwt_authz_provider());
    let service = TraitBasedAuthService::new(auth, authz);

    let debug = format!("{service:?}");

    assert!(debug.contains("TraitBasedAuthService"));
    assert!(debug.contains("AuthProvider"));
    assert!(debug.contains("AuthorizationProvider"));
}

// ============================================================================
// JwtValidationProviderImpl Tests
// ============================================================================

fn create_jwt_validation_provider() -> JwtValidationProviderImpl {
    JwtValidationProviderImpl::new(
        TEST_SECRET.to_string(),
        TEST_ISSUER.to_string(),
        default_audiences(),
    )
}

#[test]
fn test_jwt_validation_provider_new() {
    let provider = create_jwt_validation_provider();
    let debug = format!("{provider:?}");
    assert!(debug.contains("JwtValidationProviderImpl"));
}

#[test]
fn test_jwt_validation_provider_validate_token_success() {
    let provider = create_jwt_validation_provider();
    let token = create_valid_token();

    let result = provider.validate_token(&token);

    let claims = result.expect("valid token should succeed");
    assert_eq!(claims.subject, "550e8400-e29b-41d4-a716-446655440000");
    assert_eq!(claims.username, "testuser");
    assert_eq!(claims.user_type, "user");
}

#[test]
fn test_jwt_validation_provider_validate_token_expired() {
    let provider = create_jwt_validation_provider();
    let test_claims = create_test_claims(-3600, TEST_ISSUER, &["api"]);
    let token = create_test_token(&test_claims, TEST_SECRET);

    let result = provider.validate_token(&token);

    let err = result.unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("Invalid token") || msg.contains("expired"),
        "expected token error, got: {msg}"
    );
}

#[test]
fn test_jwt_validation_provider_validate_token_invalid() {
    let provider = create_jwt_validation_provider();

    let result = provider.validate_token("completely-invalid-token");

    let err = result.unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("Invalid token"), "expected InvalidToken error, got: {msg}");
}

#[test]
fn test_jwt_validation_provider_validate_token_extracts_is_admin() {
    let provider = create_jwt_validation_provider();
    let mut test_claims = create_test_claims(3600, TEST_ISSUER, &["api"]);
    test_claims.scope = "admin".to_string();
    let token = create_test_token(&test_claims, TEST_SECRET);

    let claims = provider.validate_token(&token).expect("admin token should validate");

    assert!(claims.is_admin);

    let regular_token = create_valid_token();
    let regular_claims = provider.validate_token(&regular_token).expect("regular token should validate");

    assert!(!regular_claims.is_admin);
}

#[test]
fn test_jwt_validation_provider_generate_token() {
    let provider = create_jwt_validation_provider();
    let session_id = SessionId::generate();
    let params = systemprompt_traits::GenerateTokenParams::new(
        "550e8400-e29b-41d4-a716-446655440000",
        "testuser",
        session_id,
    );

    let token = provider.generate_token(params).expect("generate_token should succeed");

    assert!(!token.is_empty());

    let validated = provider.validate_token(&token).expect("generated token should be valid");
    assert_eq!(validated.username, "testuser");
}

#[test]
fn test_jwt_validation_provider_generate_token_with_audiences() {
    let provider = JwtValidationProviderImpl::new(
        TEST_SECRET.to_string(),
        TEST_ISSUER.to_string(),
        vec![JwtAudience::Api, JwtAudience::Mcp],
    );
    let session_id = SessionId::generate();
    let params = systemprompt_traits::GenerateTokenParams::new(
        "550e8400-e29b-41d4-a716-446655440000",
        "testuser",
        session_id,
    )
    .with_audiences(vec!["api".to_string(), "mcp".to_string()]);

    let token = provider.generate_token(params).expect("generate_token should succeed");

    let validated = provider.validate_token(&token).expect("generated token should be valid");
    assert!(validated.audiences.iter().any(|a| a == "api"));
    assert!(validated.audiences.iter().any(|a| a == "mcp"));
}

#[test]
fn test_jwt_validation_provider_generate_secure_token() {
    let provider = create_jwt_validation_provider();

    let token = provider.generate_secure_token("sp");

    assert!(token.starts_with("sp_"), "secure token should start with prefix, got: {token}");
    assert!(token.len() > 5);
}
