//! Unit tests for Environment and OAuthProvider enums
//!
//! Tests cover:
//! - Environment enum variants, default, and api_url method
//! - OAuthProvider enum variants, as_str, and display_name methods

use systemprompt_cloud::{Environment, OAuthProvider, PRODUCTION_URL, SANDBOX_URL};

// ============================================================================
// Environment Tests
// ============================================================================

#[test]
fn test_environment_default_is_production() {
    let env = Environment::default();
    assert_eq!(env, Environment::Production);
}

#[test]
fn test_environment_variants() {
    let prod = Environment::Production;
    let sandbox = Environment::Sandbox;

    assert_ne!(prod, sandbox);
}

#[test]
fn test_environment_production_api_url() {
    let env = Environment::Production;
    assert_eq!(env.api_url(), PRODUCTION_URL);
    assert_eq!(env.api_url(), "https://api.systemprompt.io");
}

#[test]
fn test_environment_sandbox_api_url() {
    let env = Environment::Sandbox;
    assert_eq!(env.api_url(), SANDBOX_URL);
    assert_eq!(env.api_url(), "https://api-sandbox.systemprompt.io");
}

#[test]
fn test_environment_api_urls_different() {
    assert_ne!(
        Environment::Production.api_url(),
        Environment::Sandbox.api_url()
    );
}

#[test]
fn test_environment_debug() {
    let debug_str = format!("{:?}", Environment::Production);
    assert!(debug_str.contains("Production"));

    let debug_str = format!("{:?}", Environment::Sandbox);
    assert!(debug_str.contains("Sandbox"));
}

#[test]
fn test_environment_clone() {
    let original = Environment::Production;
    let cloned = original.clone();
    assert_eq!(original, cloned);
}

#[test]
fn test_environment_copy() {
    let original = Environment::Sandbox;
    let copied = original;
    // Both should still be valid (Copy trait)
    assert_eq!(original, copied);
}

// ============================================================================
// OAuthProvider Tests
// ============================================================================

#[test]
fn test_oauth_provider_github_as_str() {
    let provider = OAuthProvider::Github;
    assert_eq!(provider.as_str(), "github");
}

#[test]
fn test_oauth_provider_google_as_str() {
    let provider = OAuthProvider::Google;
    assert_eq!(provider.as_str(), "google");
}

#[test]
fn test_oauth_provider_github_display_name() {
    let provider = OAuthProvider::Github;
    assert_eq!(provider.display_name(), "GitHub");
}

#[test]
fn test_oauth_provider_google_display_name() {
    let provider = OAuthProvider::Google;
    assert_eq!(provider.display_name(), "Google");
}

#[test]
fn test_oauth_provider_as_str_lowercase() {
    // as_str should return lowercase for URL construction
    assert_eq!(OAuthProvider::Github.as_str(), "github");
    assert_eq!(OAuthProvider::Google.as_str(), "google");
}

#[test]
fn test_oauth_provider_display_name_proper_case() {
    // display_name should return proper capitalization for UI
    assert!(OAuthProvider::Github.display_name().contains("Git"));
    assert!(OAuthProvider::Google.display_name().starts_with('G'));
}

#[test]
fn test_oauth_provider_variants_different() {
    assert_ne!(OAuthProvider::Github, OAuthProvider::Google);
}

#[test]
fn test_oauth_provider_debug() {
    let debug_str = format!("{:?}", OAuthProvider::Github);
    assert!(debug_str.contains("Github"));

    let debug_str = format!("{:?}", OAuthProvider::Google);
    assert!(debug_str.contains("Google"));
}

#[test]
fn test_oauth_provider_clone() {
    let original = OAuthProvider::Github;
    let cloned = original.clone();
    assert_eq!(original, cloned);
}

#[test]
fn test_oauth_provider_copy() {
    let original = OAuthProvider::Google;
    let copied = original;
    // Both should still be valid (Copy trait)
    assert_eq!(original, copied);
}

#[test]
fn test_oauth_provider_equality() {
    let github1 = OAuthProvider::Github;
    let github2 = OAuthProvider::Github;
    assert_eq!(github1, github2);

    let google1 = OAuthProvider::Google;
    let google2 = OAuthProvider::Google;
    assert_eq!(google1, google2);
}
