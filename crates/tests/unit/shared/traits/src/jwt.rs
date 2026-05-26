use systemprompt_identifiers::{SessionId, UserId};
use systemprompt_traits::jwt::{AgentJwtClaims, GenerateTokenParams, JwtProviderError};

fn claims() -> AgentJwtClaims {
    AgentJwtClaims {
        subject: "user-1".to_owned(),
        username: "alice".to_owned(),
        user_type: "user".to_owned(),
        audiences: vec!["api".to_owned(), "mcp".to_owned()],
        permissions: vec!["user".to_owned(), "read:files".to_owned()],
        is_admin: false,
        expires_at: 0,
        issued_at: 0,
    }
}

#[test]
fn agent_jwt_claims_has_audience_matches() {
    let c = claims();
    assert!(c.has_audience("api"));
    assert!(c.has_audience("mcp"));
    assert!(!c.has_audience("web"));
}

#[test]
fn agent_jwt_claims_has_permission_matches() {
    let c = claims();
    assert!(c.has_permission("user"));
    assert!(c.has_permission("read:files"));
    assert!(!c.has_permission("admin"));
}

#[test]
fn jwt_provider_error_displays_have_useful_text() {
    assert_eq!(JwtProviderError::InvalidToken.to_string(), "Invalid token");
    assert_eq!(JwtProviderError::TokenExpired.to_string(), "Token expired");
    let e = JwtProviderError::MissingAudience("api".to_owned());
    assert!(e.to_string().contains("api"));
    let e = JwtProviderError::ConfigurationError("missing key".to_owned());
    assert!(e.to_string().contains("missing key"));
    let e = JwtProviderError::Internal("boom".to_owned());
    assert!(e.to_string().contains("boom"));
}

#[test]
fn generate_token_params_new_defaults() {
    let p = GenerateTokenParams::new(UserId::new("u"), "alice", SessionId::new("s"));
    assert_eq!(p.username, "alice");
    assert_eq!(p.user_type, "user");
    assert!(p.permissions.is_empty());
    assert!(p.audiences.is_empty());
    assert!(p.expires_in_hours.is_none());
}

#[test]
fn generate_token_params_builders_chain() {
    let p = GenerateTokenParams::new(UserId::new("u"), "bob", SessionId::new("s"))
        .with_user_type("admin")
        .with_permissions(vec!["admin".to_owned()])
        .with_audiences(vec!["api".to_owned(), "internal".to_owned()])
        .with_expires_in_hours(24);
    assert_eq!(p.user_type, "admin");
    assert_eq!(p.permissions, vec!["admin".to_owned()]);
    assert_eq!(p.audiences.len(), 2);
    assert_eq!(p.expires_in_hours, Some(24));
}
