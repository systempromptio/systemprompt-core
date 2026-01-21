//! Tests for OAuth types: GrantType, PkceMethod, ResponseType, ResponseMode, DisplayMode, Prompt, TokenAuthMethod

use std::str::FromStr;
use systemprompt_oauth::{
    DisplayMode, GrantType, PkceMethod, Prompt, ResponseMode, ResponseType, TokenAuthMethod,
};

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

// ============================================================================
// DisplayMode Tests
// ============================================================================

#[test]
fn test_display_mode_page_as_str() {
    assert_eq!(DisplayMode::Page.as_str(), "page");
}

#[test]
fn test_display_mode_popup_as_str() {
    assert_eq!(DisplayMode::Popup.as_str(), "popup");
}

#[test]
fn test_display_mode_touch_as_str() {
    assert_eq!(DisplayMode::Touch.as_str(), "touch");
}

#[test]
fn test_display_mode_wap_as_str() {
    assert_eq!(DisplayMode::Wap.as_str(), "wap");
}

#[test]
fn test_display_mode_from_str_page() {
    let mode = DisplayMode::from_str("page").unwrap();
    assert_eq!(mode, DisplayMode::Page);
}

#[test]
fn test_display_mode_from_str_popup() {
    let mode = DisplayMode::from_str("popup").unwrap();
    assert_eq!(mode, DisplayMode::Popup);
}

#[test]
fn test_display_mode_from_str_touch() {
    let mode = DisplayMode::from_str("touch").unwrap();
    assert_eq!(mode, DisplayMode::Touch);
}

#[test]
fn test_display_mode_from_str_wap() {
    let mode = DisplayMode::from_str("wap").unwrap();
    assert_eq!(mode, DisplayMode::Wap);
}

#[test]
fn test_display_mode_from_str_invalid() {
    let result = DisplayMode::from_str("mobile");
    assert!(result.is_err());
}

#[test]
fn test_display_mode_display() {
    assert_eq!(format!("{}", DisplayMode::Page), "page");
    assert_eq!(format!("{}", DisplayMode::Popup), "popup");
    assert_eq!(format!("{}", DisplayMode::Touch), "touch");
    assert_eq!(format!("{}", DisplayMode::Wap), "wap");
}

#[test]
fn test_display_mode_equality() {
    assert_eq!(DisplayMode::Page, DisplayMode::Page);
    assert_ne!(DisplayMode::Page, DisplayMode::Popup);
}

#[test]
fn test_display_mode_clone() {
    let mode = DisplayMode::Touch;
    let cloned = mode.clone();
    assert_eq!(mode, cloned);
}

#[test]
fn test_display_mode_copy() {
    let mode = DisplayMode::Wap;
    let copied = mode;
    assert_eq!(mode, copied);
}

#[test]
fn test_display_mode_debug() {
    let debug_str = format!("{:?}", DisplayMode::Page);
    assert!(debug_str.contains("Page"));
}

// ============================================================================
// Prompt Tests
// ============================================================================

#[test]
fn test_prompt_none_as_str() {
    assert_eq!(Prompt::None.as_str(), "none");
}

#[test]
fn test_prompt_login_as_str() {
    assert_eq!(Prompt::Login.as_str(), "login");
}

#[test]
fn test_prompt_consent_as_str() {
    assert_eq!(Prompt::Consent.as_str(), "consent");
}

#[test]
fn test_prompt_select_account_as_str() {
    assert_eq!(Prompt::SelectAccount.as_str(), "select_account");
}

#[test]
fn test_prompt_from_str_none() {
    let prompt = Prompt::from_str("none").unwrap();
    assert_eq!(prompt, Prompt::None);
}

#[test]
fn test_prompt_from_str_login() {
    let prompt = Prompt::from_str("login").unwrap();
    assert_eq!(prompt, Prompt::Login);
}

#[test]
fn test_prompt_from_str_consent() {
    let prompt = Prompt::from_str("consent").unwrap();
    assert_eq!(prompt, Prompt::Consent);
}

#[test]
fn test_prompt_from_str_select_account() {
    let prompt = Prompt::from_str("select_account").unwrap();
    assert_eq!(prompt, Prompt::SelectAccount);
}

#[test]
fn test_prompt_from_str_invalid() {
    let result = Prompt::from_str("create");
    assert!(result.is_err());
}

#[test]
fn test_prompt_display() {
    assert_eq!(format!("{}", Prompt::None), "none");
    assert_eq!(format!("{}", Prompt::Login), "login");
    assert_eq!(format!("{}", Prompt::Consent), "consent");
    assert_eq!(format!("{}", Prompt::SelectAccount), "select_account");
}

#[test]
fn test_prompt_equality() {
    assert_eq!(Prompt::Login, Prompt::Login);
    assert_ne!(Prompt::Login, Prompt::Consent);
}

#[test]
fn test_prompt_clone() {
    let prompt = Prompt::Consent;
    let cloned = prompt.clone();
    assert_eq!(prompt, cloned);
}

#[test]
fn test_prompt_copy() {
    let prompt = Prompt::SelectAccount;
    let copied = prompt;
    assert_eq!(prompt, copied);
}

#[test]
fn test_prompt_debug() {
    let debug_str = format!("{:?}", Prompt::Login);
    assert!(debug_str.contains("Login"));
}

// ============================================================================
// TokenAuthMethod Tests
// ============================================================================

#[test]
fn test_token_auth_method_client_secret_post_as_str() {
    assert_eq!(TokenAuthMethod::ClientSecretPost.as_str(), "client_secret_post");
}

#[test]
fn test_token_auth_method_client_secret_basic_as_str() {
    assert_eq!(TokenAuthMethod::ClientSecretBasic.as_str(), "client_secret_basic");
}

#[test]
fn test_token_auth_method_none_as_str() {
    assert_eq!(TokenAuthMethod::None.as_str(), "none");
}

#[test]
fn test_token_auth_method_from_str_client_secret_post() {
    let method = TokenAuthMethod::from_str("client_secret_post").unwrap();
    assert_eq!(method, TokenAuthMethod::ClientSecretPost);
}

#[test]
fn test_token_auth_method_from_str_client_secret_basic() {
    let method = TokenAuthMethod::from_str("client_secret_basic").unwrap();
    assert_eq!(method, TokenAuthMethod::ClientSecretBasic);
}

#[test]
fn test_token_auth_method_from_str_none() {
    let method = TokenAuthMethod::from_str("none").unwrap();
    assert_eq!(method, TokenAuthMethod::None);
}

#[test]
fn test_token_auth_method_from_str_invalid() {
    let result = TokenAuthMethod::from_str("private_key_jwt");
    assert!(result.is_err());
}

#[test]
fn test_token_auth_method_default() {
    assert_eq!(TokenAuthMethod::default(), TokenAuthMethod::ClientSecretPost);
}

#[test]
fn test_token_auth_method_display() {
    assert_eq!(format!("{}", TokenAuthMethod::ClientSecretPost), "client_secret_post");
    assert_eq!(format!("{}", TokenAuthMethod::ClientSecretBasic), "client_secret_basic");
    assert_eq!(format!("{}", TokenAuthMethod::None), "none");
}

#[test]
fn test_token_auth_method_equality() {
    assert_eq!(TokenAuthMethod::ClientSecretPost, TokenAuthMethod::ClientSecretPost);
    assert_ne!(TokenAuthMethod::ClientSecretPost, TokenAuthMethod::ClientSecretBasic);
}

#[test]
fn test_token_auth_method_clone() {
    let method = TokenAuthMethod::ClientSecretBasic;
    let cloned = method.clone();
    assert_eq!(method, cloned);
}

#[test]
fn test_token_auth_method_copy() {
    let method = TokenAuthMethod::None;
    let copied = method;
    assert_eq!(method, copied);
}

#[test]
fn test_token_auth_method_debug() {
    let debug_str = format!("{:?}", TokenAuthMethod::ClientSecretPost);
    assert!(debug_str.contains("ClientSecretPost"));
}
