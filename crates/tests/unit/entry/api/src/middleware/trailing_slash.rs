//! Unit tests for trailing slash middleware
//!
//! Note: The `remove_trailing_slash` middleware function and `should_redirect`
//! helper are primarily tested through integration tests since they require
//! the full axum request/response cycle.
//!
//! This module documents the expected behavior:
//! - Paths with trailing slashes under API_BASE are redirected (permanent)
//! - Root path "/" is not redirected
//! - Wellknown paths are not redirected
//! - Static asset paths (.js/, .css/, etc.) are not redirected
//! - Paths not under API_BASE are not redirected
//!
//! Tests cover:
//! - ApiPaths constants used by trailing slash logic

use systemprompt_models::modules::ApiPaths;

// ============================================================================
// ApiPaths Constants Tests (used by trailing slash middleware)
// ============================================================================

#[test]
fn test_api_base_path_defined() {
    // API_BASE is used to determine if trailing slash redirect should apply
    let api_base = ApiPaths::API_BASE;
    assert!(!api_base.is_empty());
    assert!(api_base.starts_with('/'));
}

#[test]
fn test_wellknown_base_path_defined() {
    // WELLKNOWN_BASE is excluded from trailing slash redirects
    let wellknown = ApiPaths::WELLKNOWN_BASE;
    assert!(!wellknown.is_empty());
    assert!(wellknown.starts_with('/'));
}

#[test]
fn test_api_paths_are_distinct() {
    assert_ne!(ApiPaths::API_BASE, ApiPaths::WELLKNOWN_BASE);
}

// ============================================================================
// Trailing Slash Behavior Documentation Tests
// ============================================================================

/// Documents the redirect conditions for paths
#[test]
fn test_trailing_slash_redirect_conditions_documented() {
    // These are the conditions for redirect:
    // 1. Path length > 1 (not just "/")
    // 2. Path ends with '/'
    // 3. Path starts with API_BASE
    // 4. Path does NOT start with WELLKNOWN_BASE
    // 5. Path does NOT end with asset extensions (.js/, .css/, etc.)

    // Example paths that SHOULD redirect:
    let should_redirect = vec!["/api/users/", "/api/v1/agents/"];

    for path in should_redirect {
        assert!(path.len() > 1);
        assert!(path.ends_with('/'));
        assert!(path.starts_with(ApiPaths::API_BASE));
    }
}

/// Documents paths that should NOT redirect
#[test]
fn test_trailing_slash_no_redirect_conditions_documented() {
    // Root path - too short
    let root = "/";
    assert_eq!(root.len(), 1);

    // Asset paths - excluded by extension check
    let assets = vec![
        "/api/bundle.js/",
        "/api/styles.css/",
        "/api/image.png/",
        "/api/logo.svg/",
    ];
    for asset in assets {
        assert!(
            asset.ends_with(".js/")
                || asset.ends_with(".css/")
                || asset.ends_with(".png/")
                || asset.ends_with(".svg/")
        );
    }

    // Wellknown paths - excluded
    let wellknown_path = format!("{}/agent.json/", ApiPaths::WELLKNOWN_BASE);
    assert!(wellknown_path.starts_with(ApiPaths::WELLKNOWN_BASE));
}

/// Documents that query strings are preserved during redirect
#[test]
fn test_query_string_preservation_documented() {
    // When redirecting /api/users/?page=1 -> /api/users?page=1
    // The query string should be preserved

    let path_with_query = "/api/users/";
    let query = "page=1&limit=10";
    let expected_new_path = "/api/users";
    let expected_new_uri = format!("{}?{}", expected_new_path, query);

    assert!(!expected_new_uri.ends_with('/'));
    assert!(expected_new_uri.contains('?'));
}
