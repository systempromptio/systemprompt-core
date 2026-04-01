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
    assert!(result.is_err());
}

#[test]
fn test_grant_type_display() {
    assert_eq!(format!("{}", GrantType::AuthorizationCode), "authorization_code");
    assert_eq!(format!("{}", GrantType::RefreshToken), "refresh_token");
    assert_eq!(format!("{}", GrantType::ClientCredentials), "client_credentials");
}

#[test]
fn test_grant_type_default_grant_types() {
    let defaults = GrantType::default_grant_types();
    assert_eq!(defaults.len(), 2);
    assert!(defaults.contains(&"authorization_code"));
    assert!(defaults.contains(&"refresh_token"));
}

#[test]
fn test_grant_type_equality() {
    assert_eq!(GrantType::AuthorizationCode, GrantType::AuthorizationCode);
    assert_ne!(GrantType::AuthorizationCode, GrantType::RefreshToken);
}

#[test]
fn test_grant_type_clone() {
    let grant_type = GrantType::AuthorizationCode;
    let cloned = grant_type.clone();
    assert_eq!(grant_type, cloned);
}

#[test]
fn test_grant_type_copy() {
    let grant_type = GrantType::RefreshToken;
    let copied = grant_type;
    assert_eq!(grant_type, copied);
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
    assert!(result.is_err());
}

#[test]
fn test_pkce_method_display() {
    assert_eq!(format!("{}", PkceMethod::S256), "S256");
    assert_eq!(format!("{}", PkceMethod::Plain), "plain");
}

#[test]
fn test_pkce_method_equality() {
    assert_eq!(PkceMethod::S256, PkceMethod::S256);
    assert_ne!(PkceMethod::S256, PkceMethod::Plain);
}

#[test]
fn test_pkce_method_clone() {
    let method = PkceMethod::S256;
    let cloned = method.clone();
    assert_eq!(method, cloned);
}

#[test]
fn test_pkce_method_copy() {
    let method = PkceMethod::Plain;
    let copied = method;
    assert_eq!(method, copied);
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
    assert!(result.is_err());
}

#[test]
fn test_response_type_display() {
    assert_eq!(format!("{}", ResponseType::Code), "code");
}

#[test]
fn test_response_type_equality() {
    assert_eq!(ResponseType::Code, ResponseType::Code);
}

#[test]
fn test_response_type_clone() {
    let response_type = ResponseType::Code;
    let cloned = response_type.clone();
    assert_eq!(response_type, cloned);
}

#[test]
fn test_response_type_copy() {
    let response_type = ResponseType::Code;
    let copied = response_type;
    assert_eq!(response_type, copied);
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
    assert!(result.is_err());
}

#[test]
fn test_response_mode_display() {
    assert_eq!(format!("{}", ResponseMode::Query), "query");
    assert_eq!(format!("{}", ResponseMode::Fragment), "fragment");
}

#[test]
fn test_response_mode_equality() {
    assert_eq!(ResponseMode::Query, ResponseMode::Query);
    assert_ne!(ResponseMode::Query, ResponseMode::Fragment);
}

#[test]
fn test_response_mode_clone() {
    let mode = ResponseMode::Query;
    let cloned = mode.clone();
    assert_eq!(mode, cloned);
}

#[test]
fn test_response_mode_copy() {
    let mode = ResponseMode::Fragment;
    let copied = mode;
    assert_eq!(mode, copied);
}

#[test]
fn test_response_mode_debug() {
    let debug_str = format!("{:?}", ResponseMode::Query);
    assert!(debug_str.contains("Query"));
}
