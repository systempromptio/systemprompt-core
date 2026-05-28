//! Tests for `JwtValidationProviderImpl`.
//!
//! `validate_token` and `generate_token` need a populated signing-key
//! authority and a global `Config`, both of which are wired only in
//! the full runtime. Coverage here focuses on the pure paths:
//! construction, debug formatting, and the `generate_secure_token`
//! trait method.

use systemprompt_models::auth::JwtAudience;
use systemprompt_oauth::JwtValidationProviderImpl;
use systemprompt_traits::JwtValidationProvider;

#[test]
fn new_constructs_with_issuer_and_audiences() {
    let provider = JwtValidationProviderImpl::new(
        "https://issuer.test".to_string(),
        vec![JwtAudience::Api, JwtAudience::Web],
    );
    let debug = format!("{:?}", provider);

    assert!(debug.contains("JwtValidationProviderImpl"));
    assert!(debug.contains("https://issuer.test"));
}

#[test]
fn new_accepts_empty_audiences() {
    let provider = JwtValidationProviderImpl::new("issuer".to_string(), Vec::new());

    let debug = format!("{:?}", provider);
    assert!(debug.contains("issuer"));
}

#[test]
fn generate_secure_token_delegates_to_helper() {
    let provider = JwtValidationProviderImpl::new("issuer".to_string(), vec![JwtAudience::Api]);

    let token = provider.generate_secure_token("auth");

    assert!(token.starts_with("auth_"));
    assert!(token.len() > "auth_".len());
}

#[test]
fn generate_secure_token_returns_unique_values() {
    let provider = JwtValidationProviderImpl::new("issuer".to_string(), Vec::new());

    let a = provider.generate_secure_token("tok");
    let b = provider.generate_secure_token("tok");

    assert_ne!(a, b);
}

#[test]
fn validate_token_rejects_malformed_input() {
    let provider = JwtValidationProviderImpl::new(
        "https://issuer.test".to_string(),
        vec![JwtAudience::Api],
    );

    let err = provider
        .validate_token("not.a.valid.jwt")
        .expect_err("malformed token must be rejected");

    let msg = err.to_string();
    assert!(
        msg.to_lowercase().contains("invalid") || msg.to_lowercase().contains("token"),
        "unexpected error message: {msg}"
    );
}

#[test]
fn validate_token_rejects_empty_string() {
    let provider =
        JwtValidationProviderImpl::new("issuer".to_string(), vec![JwtAudience::Api]);

    let err = provider
        .validate_token("")
        .expect_err("empty token must be rejected");
    let _ = err.to_string();
}
