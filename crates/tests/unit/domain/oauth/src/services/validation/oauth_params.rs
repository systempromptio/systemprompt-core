//! Tests for OAuth parameter validation

use systemprompt_core_oauth::services::validation::{
    get_audit_user, optional_param, required_param, scope_param, validate_pkce, CsrfToken,
    ValidatedClientRegistration,
};
use systemprompt_models::{AuthError, GrantType, ResponseType};

// ============================================================================
// required_param Tests
// ============================================================================

#[test]
fn test_required_param_success() {
    let result = required_param(Some("value"), "param_name");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "value");
}

#[test]
fn test_required_param_none() {
    let result = required_param(None, "client_id");
    assert!(result.is_err());
    match result.unwrap_err() {
        AuthError::InvalidRequest { reason } => {
            assert!(reason.contains("client_id"));
            assert!(reason.contains("required"));
        }
        _ => panic!("Expected InvalidRequest error"),
    }
}

#[test]
fn test_required_param_empty_string() {
    let result = required_param(Some(""), "scope");
    assert!(result.is_err());
    match result.unwrap_err() {
        AuthError::InvalidRequest { reason } => {
            assert!(reason.contains("scope"));
            assert!(reason.contains("required"));
        }
        _ => panic!("Expected InvalidRequest error"),
    }
}

#[test]
fn test_required_param_whitespace() {
    // Whitespace-only string should be treated as valid content
    let result = required_param(Some("  "), "param");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "  ");
}

#[test]
fn test_required_param_preserves_value() {
    let result = required_param(Some("test_value_123"), "test_param");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "test_value_123");
}

// ============================================================================
// optional_param Tests
// ============================================================================

#[test]
fn test_optional_param_with_value() {
    let result = optional_param(Some("value"));
    assert_eq!(result, Some("value".to_string()));
}

#[test]
fn test_optional_param_none() {
    let result = optional_param(None);
    assert!(result.is_none());
}

#[test]
fn test_optional_param_empty_string() {
    let result = optional_param(Some(""));
    assert!(result.is_none());
}

#[test]
fn test_optional_param_whitespace() {
    // Whitespace-only should be treated as valid value
    let result = optional_param(Some("  "));
    assert_eq!(result, Some("  ".to_string()));
}

#[test]
fn test_optional_param_special_chars() {
    let result = optional_param(Some("value!@#$%"));
    assert_eq!(result, Some("value!@#$%".to_string()));
}

// ============================================================================
// scope_param Tests
// ============================================================================

#[test]
fn test_scope_param_single_scope() {
    let result = scope_param(Some("openid"));
    assert!(result.is_ok());
    let scopes = result.unwrap();
    assert_eq!(scopes.len(), 1);
    assert_eq!(scopes[0], "openid");
}

#[test]
fn test_scope_param_multiple_scopes() {
    let result = scope_param(Some("openid profile email"));
    assert!(result.is_ok());
    let scopes = result.unwrap();
    assert_eq!(scopes.len(), 3);
    assert!(scopes.contains(&"openid".to_string()));
    assert!(scopes.contains(&"profile".to_string()));
    assert!(scopes.contains(&"email".to_string()));
}

#[test]
fn test_scope_param_extra_whitespace() {
    let result = scope_param(Some("  openid   profile   "));
    assert!(result.is_ok());
    let scopes = result.unwrap();
    assert_eq!(scopes.len(), 2);
    assert!(scopes.contains(&"openid".to_string()));
    assert!(scopes.contains(&"profile".to_string()));
}

#[test]
fn test_scope_param_none() {
    let result = scope_param(None);
    assert!(result.is_err());
    match result.unwrap_err() {
        AuthError::InvalidRequest { reason } => {
            assert!(reason.contains("scope"));
            assert!(reason.contains("required"));
        }
        _ => panic!("Expected InvalidRequest error"),
    }
}

#[test]
fn test_scope_param_empty_string() {
    let result = scope_param(Some(""));
    assert!(result.is_err());
}

#[test]
fn test_scope_param_whitespace_only() {
    let result = scope_param(Some("   "));
    assert!(result.is_err());
    match result.unwrap_err() {
        AuthError::InvalidScope { scope } => {
            assert_eq!(scope.trim(), "");
        }
        _ => panic!("Expected InvalidScope error"),
    }
}

#[test]
fn test_scope_param_tabs_and_newlines() {
    let result = scope_param(Some("openid\tprofile\nemail"));
    assert!(result.is_ok());
    let scopes = result.unwrap();
    assert_eq!(scopes.len(), 3);
}

// ============================================================================
// validate_pkce Tests
// ============================================================================

#[test]
fn test_validate_pkce_s256() {
    let result = validate_pkce(Some("challenge_value"), Some("S256"));
    assert!(result.is_ok());
    let pkce = result.unwrap();
    assert_eq!(pkce.challenge, "challenge_value");
}

#[test]
fn test_validate_pkce_plain_rejected() {
    // "plain" PKCE method is rejected as weak
    let result = validate_pkce(Some("plain_challenge"), Some("plain"));
    assert!(result.is_err());
    match result.unwrap_err() {
        AuthError::WeakPkceMethod { method } => {
            assert_eq!(method, "plain");
        }
        _ => panic!("Expected WeakPkceMethod error"),
    }
}

#[test]
fn test_validate_pkce_missing_challenge() {
    let result = validate_pkce(None, Some("S256"));
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), AuthError::MissingCodeChallenge));
}

#[test]
fn test_validate_pkce_empty_challenge() {
    let result = validate_pkce(Some(""), Some("S256"));
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), AuthError::MissingCodeChallenge));
}

#[test]
fn test_validate_pkce_missing_method() {
    let result = validate_pkce(Some("challenge"), None);
    assert!(result.is_err());
    match result.unwrap_err() {
        AuthError::InvalidRequest { reason } => {
            assert!(reason.contains("code_challenge_method"));
        }
        _ => panic!("Expected InvalidRequest error"),
    }
}

#[test]
fn test_validate_pkce_invalid_method() {
    let result = validate_pkce(Some("challenge"), Some("sha256"));
    assert!(result.is_err());
}

#[test]
fn test_validate_pkce_debug() {
    let result = validate_pkce(Some("challenge"), Some("S256")).unwrap();
    let debug_str = format!("{:?}", result);
    assert!(debug_str.contains("challenge"));
    assert!(debug_str.contains("PkceChallenge"));
}

// ============================================================================
// get_audit_user Tests
// ============================================================================

#[test]
fn test_get_audit_user_success() {
    let result = get_audit_user(Some("user_123"));
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "user_123");
}

#[test]
fn test_get_audit_user_none() {
    let result = get_audit_user(None);
    assert!(result.is_err());
    match result.unwrap_err() {
        AuthError::InvalidRequest { reason } => {
            assert!(reason.contains("Authenticated user required"));
        }
        _ => panic!("Expected InvalidRequest error"),
    }
}

#[test]
fn test_get_audit_user_empty() {
    let result = get_audit_user(Some(""));
    assert!(result.is_err());
    match result.unwrap_err() {
        AuthError::InvalidRequest { reason } => {
            assert!(reason.contains("Authenticated user required"));
        }
        _ => panic!("Expected InvalidRequest error"),
    }
}

#[test]
fn test_get_audit_user_whitespace() {
    // Whitespace-only is valid because it's not empty after the filter check
    let result = get_audit_user(Some("  "));
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "  ");
}

#[test]
fn test_get_audit_user_uuid_format() {
    let result = get_audit_user(Some("550e8400-e29b-41d4-a716-446655440000"));
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "550e8400-e29b-41d4-a716-446655440000");
}

// ============================================================================
// CsrfToken Tests
// ============================================================================

#[test]
fn test_csrf_token_new_success() {
    let result = CsrfToken::new("valid_state_123");
    assert!(result.is_ok());
}

#[test]
fn test_csrf_token_as_str() {
    let token = CsrfToken::new("my_state").unwrap();
    assert_eq!(token.as_str(), "my_state");
}

#[test]
fn test_csrf_token_into_string() {
    let token = CsrfToken::new("state_to_consume").unwrap();
    let s: String = token.into_string();
    assert_eq!(s, "state_to_consume");
}

#[test]
fn test_csrf_token_empty() {
    let result = CsrfToken::new("");
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), AuthError::MissingState));
}

#[test]
fn test_csrf_token_with_hyphen() {
    let result = CsrfToken::new("state-with-hyphens");
    assert!(result.is_ok());
    assert_eq!(result.unwrap().as_str(), "state-with-hyphens");
}

#[test]
fn test_csrf_token_with_underscore() {
    let result = CsrfToken::new("state_with_underscores");
    assert!(result.is_ok());
    assert_eq!(result.unwrap().as_str(), "state_with_underscores");
}

#[test]
fn test_csrf_token_alphanumeric_only() {
    let result = CsrfToken::new("ABC123xyz");
    assert!(result.is_ok());
}

#[test]
fn test_csrf_token_invalid_special_chars() {
    let result = CsrfToken::new("state!@#$");
    assert!(result.is_err());
    match result.unwrap_err() {
        AuthError::InvalidRequest { reason } => {
            assert!(reason.contains("alphanumeric"));
        }
        _ => panic!("Expected InvalidRequest error"),
    }
}

#[test]
fn test_csrf_token_with_space() {
    let result = CsrfToken::new("state with space");
    assert!(result.is_err());
}

#[test]
fn test_csrf_token_with_dot() {
    let result = CsrfToken::new("state.with.dot");
    assert!(result.is_err());
}

#[test]
fn test_csrf_token_from_string() {
    let state = String::from("string_state");
    let result = CsrfToken::new(state);
    assert!(result.is_ok());
    assert_eq!(result.unwrap().as_str(), "string_state");
}

#[test]
fn test_csrf_token_debug() {
    let token = CsrfToken::new("debug_state").unwrap();
    let debug_str = format!("{:?}", token);
    assert!(debug_str.contains("CsrfToken"));
    assert!(debug_str.contains("debug_state"));
}

// ============================================================================
// PkceChallenge Tests
// ============================================================================

#[test]
fn test_pkce_challenge_debug() {
    let result = validate_pkce(Some("challenge123"), Some("S256")).unwrap();
    let debug_str = format!("{:?}", result);
    assert!(debug_str.contains("PkceChallenge"));
    assert!(debug_str.contains("challenge123"));
}

#[test]
fn test_pkce_challenge_fields() {
    let result = validate_pkce(Some("my_challenge"), Some("S256")).unwrap();
    assert_eq!(result.challenge, "my_challenge");
    // method should be S256
    assert_eq!(format!("{:?}", result.method), "S256");
}

// ============================================================================
// ValidatedClientRegistration Tests
// ============================================================================

#[test]
fn test_validated_client_registration_creation() {
    let registration = ValidatedClientRegistration {
        client_name: "Test Client".to_string(),
        redirect_uris: vec!["https://example.com/callback".to_string()],
        grant_types: vec![GrantType::AuthorizationCode],
        response_types: vec![ResponseType::Code],
    };

    assert_eq!(registration.client_name, "Test Client");
    assert_eq!(registration.redirect_uris.len(), 1);
    assert_eq!(registration.grant_types.len(), 1);
    assert_eq!(registration.response_types.len(), 1);
}

#[test]
fn test_validated_client_registration_multiple_values() {
    let registration = ValidatedClientRegistration {
        client_name: "Multi Client".to_string(),
        redirect_uris: vec![
            "https://example.com/callback1".to_string(),
            "https://example.com/callback2".to_string(),
        ],
        grant_types: vec![GrantType::AuthorizationCode, GrantType::RefreshToken],
        response_types: vec![ResponseType::Code, ResponseType::Token],
    };

    assert_eq!(registration.redirect_uris.len(), 2);
    assert_eq!(registration.grant_types.len(), 2);
    assert_eq!(registration.response_types.len(), 2);
}

#[test]
fn test_validated_client_registration_debug() {
    let registration = ValidatedClientRegistration {
        client_name: "Debug Client".to_string(),
        redirect_uris: vec!["https://example.com/callback".to_string()],
        grant_types: vec![GrantType::AuthorizationCode],
        response_types: vec![ResponseType::Code],
    };

    let debug_str = format!("{:?}", registration);
    assert!(debug_str.contains("ValidatedClientRegistration"));
    assert!(debug_str.contains("Debug Client"));
}
