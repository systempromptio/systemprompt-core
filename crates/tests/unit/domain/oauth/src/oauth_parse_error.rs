//! Tests for `OAuthParseError` — all seven display variants and the
//! `std::error::Error` impl.

use std::str::FromStr;
use systemprompt_oauth::{
    DisplayMode, GrantType, PkceMethod, Prompt, ResponseMode, ResponseType, TokenAuthMethod,
};

// -- OAuthParseError variants via FromStr failures --------------------------

#[test]
fn grant_type_parse_error_contains_unknown_text_and_input() {
    let err = GrantType::from_str("bogus_grant").unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("unknown grant type"));
    assert!(msg.contains("bogus_grant"));
}

#[test]
fn pkce_method_parse_error_contains_unknown_text_and_input() {
    let err = PkceMethod::from_str("plain").unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("unknown PKCE method"));
    assert!(msg.contains("plain"));
}

#[test]
fn response_type_parse_error_contains_unknown_text_and_input() {
    let err = ResponseType::from_str("token").unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("unknown response type"));
    assert!(msg.contains("token"));
}

#[test]
fn response_mode_parse_error_contains_unknown_text_and_input() {
    let err = ResponseMode::from_str("form_post").unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("unknown response mode"));
    assert!(msg.contains("form_post"));
}

#[test]
fn display_mode_parse_error_contains_unknown_text_and_input() {
    let err = DisplayMode::from_str("embed").unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("unknown display mode"));
    assert!(msg.contains("embed"));
}

#[test]
fn prompt_parse_error_contains_unknown_text_and_input() {
    let err = Prompt::from_str("force_login").unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("unknown prompt type"));
    assert!(msg.contains("force_login"));
}

#[test]
fn token_auth_method_parse_error_contains_unknown_text_and_input() {
    let err = TokenAuthMethod::from_str("private_key_jwt").unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("unknown token auth method"));
    assert!(msg.contains("private_key_jwt"));
}

#[test]
fn oauth_parse_error_implements_std_error() {
    let err = GrantType::from_str("nope").unwrap_err();
    let _boxed: Box<dyn std::error::Error> = Box::new(err);
}

#[test]
fn oauth_parse_error_debug_includes_variant_name() {
    let err = GrantType::from_str("nope").unwrap_err();
    let dbg = format!("{:?}", err);
    assert!(dbg.contains("GrantType"));
}

// -- Token-exchange grant (URN form) ----------------------------------------

#[test]
fn grant_type_token_exchange_as_str_is_urn() {
    assert_eq!(
        GrantType::TokenExchange.as_str(),
        "urn:ietf:params:oauth:grant-type:token-exchange"
    );
}

#[test]
fn grant_type_token_exchange_from_str_succeeds() {
    let g = GrantType::from_str("urn:ietf:params:oauth:grant-type:token-exchange").unwrap();
    assert_eq!(g, GrantType::TokenExchange);
}

#[test]
fn grant_type_token_exchange_display() {
    assert_eq!(
        format!("{}", GrantType::TokenExchange),
        "urn:ietf:params:oauth:grant-type:token-exchange"
    );
}

#[test]
fn grant_type_token_exchange_debug() {
    let dbg = format!("{:?}", GrantType::TokenExchange);
    assert!(dbg.contains("TokenExchange"));
}

#[test]
fn grant_type_default_grant_types_excludes_token_exchange() {
    let defaults = GrantType::default_grant_types();
    assert!(!defaults.contains(&"urn:ietf:params:oauth:grant-type:token-exchange"));
}

// -- Equality and Copy for all parsed enum values ---------------------------

#[test]
fn grant_type_eq_same_variants() {
    assert_eq!(GrantType::AuthorizationCode, GrantType::AuthorizationCode);
    assert_ne!(GrantType::AuthorizationCode, GrantType::RefreshToken);
}

#[test]
fn pkce_method_copy_preserves_variant() {
    let a = PkceMethod::S256;
    let b = a;
    assert_eq!(a, b);
}

#[test]
fn response_type_eq() {
    assert_eq!(ResponseType::Code, ResponseType::Code);
}

#[test]
fn response_mode_eq() {
    assert_eq!(ResponseMode::Query, ResponseMode::Query);
    assert_ne!(ResponseMode::Query, ResponseMode::Fragment);
}

#[test]
fn display_mode_all_variants_round_trip() {
    for (input, expected) in [
        ("page", DisplayMode::Page),
        ("popup", DisplayMode::Popup),
        ("touch", DisplayMode::Touch),
        ("wap", DisplayMode::Wap),
    ] {
        let parsed = DisplayMode::from_str(input).unwrap();
        assert_eq!(parsed, expected);
        assert_eq!(parsed.as_str(), input);
    }
}

#[test]
fn prompt_all_variants_round_trip() {
    for (input, expected) in [
        ("none", Prompt::None),
        ("login", Prompt::Login),
        ("consent", Prompt::Consent),
        ("select_account", Prompt::SelectAccount),
    ] {
        let parsed = Prompt::from_str(input).unwrap();
        assert_eq!(parsed, expected);
        assert_eq!(parsed.as_str(), input);
    }
}

#[test]
fn token_auth_method_default_is_client_secret_post() {
    let d = TokenAuthMethod::default();
    assert_eq!(d, TokenAuthMethod::ClientSecretPost);
}
