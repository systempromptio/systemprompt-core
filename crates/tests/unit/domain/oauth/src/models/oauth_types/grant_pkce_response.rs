//! Tests for GrantType, PkceMethod, ResponseType, and ResponseMode.

use std::str::FromStr;
use systemprompt_oauth::{GrantType, PkceMethod, ResponseMode, ResponseType};

// ============================================================================
// GrantType Tests
// ============================================================================

#[test]
fn test_grant_type_authorization_code_as_str() {
    assert_eq!(GrantType::AuthorizationCode.as_str(), "authorization_code");
}

#[test]
fn test_grant_type_refresh_token_as_str() {
    assert_eq!(GrantType::RefreshToken.as_str(), "refresh_token");
}

#[test]
fn test_grant_type_client_credentials_as_str() {
    assert_eq!(GrantType::ClientCredentials.as_str(), "client_credentials");
}

#[test]
fn test_grant_type_from_str_authorization_code() {
    let grant_type = GrantType::from_str("authorization_code").unwrap();
    assert_eq!(grant_type, GrantType::AuthorizationCode);
}

#[test]
fn test_grant_type_from_str_refresh_token() {
    let grant_type = GrantType::from_str("refresh_token").unwrap();
    assert_eq!(grant_type, GrantType::RefreshToken);
}

#[test]
fn test_grant_type_from_str_client_credentials() {
    let grant_type = GrantType::from_str("client_credentials").unwrap();
    assert_eq!(grant_type, GrantType::ClientCredentials);
}

#[test]
fn test_grant_type_from_str_invalid() {
    let result = GrantType::from_str("invalid_grant");
    result.unwrap_err();
}

#[test]
fn test_grant_type_display() {
    assert_eq!(
        format!("{}", GrantType::AuthorizationCode),
        "authorization_code"
    );
    assert_eq!(format!("{}", GrantType::RefreshToken), "refresh_token");
    assert_eq!(
        format!("{}", GrantType::ClientCredentials),
        "client_credentials"
    );
}

#[test]
fn test_grant_type_default_grant_types() {
    let defaults = GrantType::default_grant_types();
    assert_eq!(defaults.len(), 2);
    assert!(defaults.contains(&"authorization_code"));
    assert!(defaults.contains(&"refresh_token"));
}

#[test]
fn test_grant_type_debug() {
    let debug_str = format!("{:?}", GrantType::AuthorizationCode);
    assert!(debug_str.contains("AuthorizationCode"));
}

// ============================================================================
// PkceMethod Tests
// ============================================================================

#[test]
fn test_pkce_method_s256_as_str() {
    assert_eq!(PkceMethod::S256.as_str(), "S256");
}

#[test]
fn test_pkce_method_plain_as_str() {
    assert_eq!(PkceMethod::Plain.as_str(), "plain");
}

#[test]
fn test_pkce_method_from_str_s256() {
    let method = PkceMethod::from_str("S256").unwrap();
    assert_eq!(method, PkceMethod::S256);
}

#[test]
fn test_pkce_method_from_str_plain() {
    let method = PkceMethod::from_str("plain").unwrap();
    assert_eq!(method, PkceMethod::Plain);
}

#[test]
fn test_pkce_method_from_str_invalid() {
    let result = PkceMethod::from_str("sha256");
    result.unwrap_err();
}

#[test]
fn test_pkce_method_display() {
    assert_eq!(format!("{}", PkceMethod::S256), "S256");
    assert_eq!(format!("{}", PkceMethod::Plain), "plain");
}

#[test]
fn test_pkce_method_debug() {
    let debug_str = format!("{:?}", PkceMethod::S256);
    assert!(debug_str.contains("S256"));
}

// ============================================================================
// ResponseType Tests
// ============================================================================

#[test]
fn test_response_type_code_as_str() {
    assert_eq!(ResponseType::Code.as_str(), "code");
}

#[test]
fn test_response_type_from_str_code() {
    let response_type = ResponseType::from_str("code").unwrap();
    assert_eq!(response_type, ResponseType::Code);
}

#[test]
fn test_response_type_from_str_invalid() {
    let result = ResponseType::from_str("token");
    result.unwrap_err();
}

#[test]
fn test_response_type_display() {
    assert_eq!(format!("{}", ResponseType::Code), "code");
}

#[test]
fn test_response_type_debug() {
    let debug_str = format!("{:?}", ResponseType::Code);
    assert!(debug_str.contains("Code"));
}

// ============================================================================
// ResponseMode Tests
// ============================================================================

#[test]
fn test_response_mode_query_as_str() {
    assert_eq!(ResponseMode::Query.as_str(), "query");
}

#[test]
fn test_response_mode_fragment_as_str() {
    assert_eq!(ResponseMode::Fragment.as_str(), "fragment");
}

#[test]
fn test_response_mode_from_str_query() {
    let mode = ResponseMode::from_str("query").unwrap();
    assert_eq!(mode, ResponseMode::Query);
}

#[test]
fn test_response_mode_from_str_fragment() {
    let mode = ResponseMode::from_str("fragment").unwrap();
    assert_eq!(mode, ResponseMode::Fragment);
}

#[test]
fn test_response_mode_from_str_invalid() {
    let result = ResponseMode::from_str("form_post");
    result.unwrap_err();
}

#[test]
fn test_response_mode_display() {
    assert_eq!(format!("{}", ResponseMode::Query), "query");
    assert_eq!(format!("{}", ResponseMode::Fragment), "fragment");
}

#[test]
fn test_response_mode_debug() {
    let debug_str = format!("{:?}", ResponseMode::Query);
    assert!(debug_str.contains("Query"));
}
