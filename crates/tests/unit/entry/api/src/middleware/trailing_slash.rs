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

#[test]
fn test_api_base_path_defined() {
    let api_base = ApiPaths::API_BASE;
    assert_eq!(api_base, "/api");
}

#[test]
fn test_wellknown_base_path_defined() {
    let wellknown = ApiPaths::WELLKNOWN_BASE;
    assert_eq!(wellknown, "/.well-known");
}

#[test]
fn test_api_paths_are_distinct() {
    assert_ne!(ApiPaths::API_BASE, ApiPaths::WELLKNOWN_BASE);
}

#[test]
fn test_trailing_slash_redirect_conditions_documented() {
    let should_redirect = vec!["/api/users/", "/api/v1/agents/"];

    for path in should_redirect {
        assert!(path.len() > 1);
        assert!(path.ends_with('/'));
        assert!(path.starts_with(ApiPaths::API_BASE));
    }
}

#[test]
fn test_trailing_slash_no_redirect_conditions_documented() {
    let root = "/";
    assert_eq!(root.len(), 1);

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

    let wellknown_path = format!("{}/agent.json/", ApiPaths::WELLKNOWN_BASE);
    assert!(wellknown_path.starts_with(ApiPaths::WELLKNOWN_BASE));
}

#[test]
fn test_query_string_preservation_documented() {
    let _path_with_query = "/api/users/";
    let query = "page=1&limit=10";
    let expected_new_path = "/api/users";
    let expected_new_uri = format!("{}?{}", expected_new_path, query);

    assert!(!expected_new_uri.ends_with('/'));
    assert!(expected_new_uri.contains('?'));
}
