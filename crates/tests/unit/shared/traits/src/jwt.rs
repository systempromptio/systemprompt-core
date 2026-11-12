use systemprompt_identifiers::UserId;
use systemprompt_traits::jwt::{AgentJwtClaims, JwtProviderError};

fn claims() -> AgentJwtClaims {
    AgentJwtClaims {
        subject: UserId::new("user-1"),
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
    let e = JwtProviderError::ConfigurationError {
        message: "missing key".to_owned(),
    };
    assert!(e.to_string().contains("missing key"));
    let e = JwtProviderError::Internal("boom".into());
    assert!(e.to_string().contains("boom"));
}
