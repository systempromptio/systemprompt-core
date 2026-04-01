//! Tests for DisplayMode, Prompt, and TokenAuthMethod.

use std::str::FromStr;
use systemprompt_oauth::{DisplayMode, Prompt, TokenAuthMethod};

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
    result.unwrap_err();
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
    result.unwrap_err();
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
    result.unwrap_err();
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
