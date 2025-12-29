//! Unit tests for agent authentication utilities
//!
//! Tests cover:
//! - Bearer token extraction
//! - JwtValidator creation and debug
//! - AgentSessionUser construction

use systemprompt_core_agent::services::shared::auth::{
    extract_bearer_token, AgentSessionUser, JwtValidator,
};

// ============================================================================
// Bearer Token Extraction Tests
// ============================================================================

#[test]
fn test_extract_bearer_token_valid() {
    let header = "Bearer abc123xyz";
    let result = extract_bearer_token(header);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "abc123xyz");
}

#[test]
fn test_extract_bearer_token_with_spaces() {
    let header = "Bearer token with spaces";
    let result = extract_bearer_token(header);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "token with spaces");
}

#[test]
fn test_extract_bearer_token_missing_prefix() {
    let header = "abc123xyz";
    let result = extract_bearer_token(header);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("invalid authorization header"));
}

#[test]
fn test_extract_bearer_token_lowercase_bearer() {
    let header = "bearer abc123xyz";
    let result = extract_bearer_token(header);
    assert!(result.is_err()); // "Bearer" is case-sensitive
}

#[test]
fn test_extract_bearer_token_basic_auth() {
    let header = "Basic dXNlcjpwYXNz";
    let result = extract_bearer_token(header);
    assert!(result.is_err());
}

#[test]
fn test_extract_bearer_token_empty_token() {
    let header = "Bearer ";
    let result = extract_bearer_token(header);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "");
}

#[test]
fn test_extract_bearer_token_empty_header() {
    let header = "";
    let result = extract_bearer_token(header);
    assert!(result.is_err());
}

// ============================================================================
// JwtValidator Tests
// ============================================================================

#[test]
fn test_jwt_validator_new() {
    let validator = JwtValidator::new("secret123".to_string());
    // Validator should be created without panic
    let debug = format!("{:?}", validator);
    assert!(debug.contains("JwtValidator"));
}

#[test]
fn test_jwt_validator_debug_hides_key() {
    let validator = JwtValidator::new("super_secret_key".to_string());
    let debug = format!("{:?}", validator);

    // Debug should not expose the actual secret key
    assert!(!debug.contains("super_secret_key"));
    assert!(debug.contains("<decoding_key>"));
}

#[test]
fn test_jwt_validator_validate_invalid_token() {
    let validator = JwtValidator::new("secret".to_string());
    let result = validator.validate_token("invalid_token");

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("invalid token"));
}

#[test]
fn test_jwt_validator_validate_malformed_token() {
    let validator = JwtValidator::new("secret".to_string());
    let result = validator.validate_token("not.a.valid.jwt.token.at.all");

    assert!(result.is_err());
}

#[test]
fn test_jwt_validator_validate_empty_token() {
    let validator = JwtValidator::new("secret".to_string());
    let result = validator.validate_token("");

    assert!(result.is_err());
}

// ============================================================================
// AgentSessionUser Tests
// ============================================================================

#[test]
fn test_agent_session_user_debug() {
    let user = AgentSessionUser {
        id: "user-123".to_string(),
        username: "testuser".to_string(),
        user_type: "registered".to_string(),
        roles: vec!["admin".to_string(), "user".to_string()],
    };

    let debug = format!("{:?}", user);
    assert!(debug.contains("AgentSessionUser"));
    assert!(debug.contains("user-123"));
    assert!(debug.contains("testuser"));
}

#[test]
fn test_agent_session_user_clone() {
    let user = AgentSessionUser {
        id: "user-clone".to_string(),
        username: "cloneuser".to_string(),
        user_type: "anonymous".to_string(),
        roles: vec!["reader".to_string()],
    };

    let cloned = user.clone();
    assert_eq!(user.id, cloned.id);
    assert_eq!(user.username, cloned.username);
    assert_eq!(user.user_type, cloned.user_type);
    assert_eq!(user.roles, cloned.roles);
}

#[test]
fn test_agent_session_user_empty_roles() {
    let user = AgentSessionUser {
        id: "user-no-roles".to_string(),
        username: "noroles".to_string(),
        user_type: "guest".to_string(),
        roles: vec![],
    };

    assert!(user.roles.is_empty());
}

#[test]
fn test_agent_session_user_multiple_roles() {
    let user = AgentSessionUser {
        id: "user-multi".to_string(),
        username: "multiuser".to_string(),
        user_type: "registered".to_string(),
        roles: vec![
            "admin".to_string(),
            "editor".to_string(),
            "viewer".to_string(),
        ],
    };

    assert_eq!(user.roles.len(), 3);
    assert!(user.roles.contains(&"editor".to_string()));
}
