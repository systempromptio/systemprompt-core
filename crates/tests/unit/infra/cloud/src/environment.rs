//! Unit tests for Environment and OAuthProvider enums

use systemprompt_cloud::{Environment, OAuthProvider};

#[test]
fn test_environment_default_is_production() {
    let default = Environment::default();
    assert_eq!(default, Environment::Production);
}

#[test]
fn test_environment_variants_are_distinct() {
    let prod = Environment::Production;
    let sandbox = Environment::Sandbox;
    assert_ne!(prod, sandbox);
}

#[test]
fn test_environment_production_api_url() {
    let prod = Environment::Production;
    let url = prod.api_url();
    assert_eq!(url, "https://api.systemprompt.io");
}

#[test]
fn test_environment_sandbox_api_url() {
    let sandbox = Environment::Sandbox;
    let url = sandbox.api_url();
    assert_eq!(url, "https://api-sandbox.systemprompt.io");
}

#[test]
fn test_environment_api_urls_are_different() {
    let prod_url = Environment::Production.api_url();
    let sandbox_url = Environment::Sandbox.api_url();
    assert_ne!(prod_url, sandbox_url);
}

#[test]
fn test_environment_debug_production() {
    let prod = Environment::Production;
    let debug_str = format!("{:?}", prod);
    assert!(debug_str.contains("Production"));
}

#[test]
fn test_environment_debug_sandbox() {
    let sandbox = Environment::Sandbox;
    let debug_str = format!("{:?}", sandbox);
    assert!(debug_str.contains("Sandbox"));
}

#[test]
fn test_environment_clone() {
    let prod = Environment::Production;
    let cloned = prod.clone();
    assert_eq!(prod, cloned);
}

#[test]
fn test_environment_copy() {
    let sandbox = Environment::Sandbox;
    let copied = sandbox;
    assert_eq!(sandbox, copied);
}

#[test]
fn test_oauth_provider_github_as_str() {
    let github = OAuthProvider::Github;
    assert_eq!(github.as_str(), "github");
}

#[test]
fn test_oauth_provider_google_as_str() {
    let google = OAuthProvider::Google;
    assert_eq!(google.as_str(), "google");
}

#[test]
fn test_oauth_provider_github_display_name() {
    let github = OAuthProvider::Github;
    assert_eq!(github.display_name(), "GitHub");
}

#[test]
fn test_oauth_provider_google_display_name() {
    let google = OAuthProvider::Google;
    assert_eq!(google.display_name(), "Google");
}

#[test]
fn test_oauth_provider_variants_are_distinct() {
    let github = OAuthProvider::Github;
    let google = OAuthProvider::Google;
    assert_ne!(github, google);
}

#[test]
fn test_oauth_provider_debug_github() {
    let github = OAuthProvider::Github;
    let debug_str = format!("{:?}", github);
    assert!(debug_str.contains("Github"));
}

#[test]
fn test_oauth_provider_debug_google() {
    let google = OAuthProvider::Google;
    let debug_str = format!("{:?}", google);
    assert!(debug_str.contains("Google"));
}

#[test]
fn test_oauth_provider_clone() {
    let github = OAuthProvider::Github;
    let cloned = github.clone();
    assert_eq!(github, cloned);
}

#[test]
fn test_oauth_provider_copy() {
    let google = OAuthProvider::Google;
    let copied = google;
    assert_eq!(google, copied);
}

#[test]
fn test_oauth_provider_as_str_is_lowercase() {
    let github = OAuthProvider::Github;
    let google = OAuthProvider::Google;
    assert_eq!(github.as_str(), github.as_str().to_lowercase());
    assert_eq!(google.as_str(), google.as_str().to_lowercase());
}

#[test]
fn test_oauth_provider_display_name_is_capitalized() {
    let github = OAuthProvider::Github;
    let google = OAuthProvider::Google;
    assert!(github.display_name().chars().next().unwrap().is_uppercase());
    assert!(google.display_name().chars().next().unwrap().is_uppercase());
}
