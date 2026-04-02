//! Unit tests for should_redirect pure function
//!
//! Tests cover:
//! - Paths with trailing slashes under API_BASE redirect
//! - Root path does not redirect
//! - Paths without trailing slashes do not redirect
//! - Wellknown paths do not redirect
//! - Static asset paths do not redirect
//! - Non-API paths do not redirect

use systemprompt_api::services::middleware::should_redirect;

#[test]
fn api_path_with_trailing_slash_redirects() {
    assert!(should_redirect("/api/users/"));
}

#[test]
fn api_v1_agents_trailing_slash_redirects() {
    assert!(should_redirect("/api/v1/agents/"));
}

#[test]
fn api_nested_path_trailing_slash_redirects() {
    assert!(should_redirect("/api/v1/core/contexts/"));
}

#[test]
fn root_path_does_not_redirect() {
    assert!(!should_redirect("/"));
}

#[test]
fn empty_path_does_not_redirect() {
    assert!(!should_redirect(""));
}

#[test]
fn api_path_without_trailing_slash_does_not_redirect() {
    assert!(!should_redirect("/api/users"));
}

#[test]
fn wellknown_path_with_trailing_slash_does_not_redirect() {
    assert!(!should_redirect("/.well-known/agent.json/"));
}

#[test]
fn js_asset_trailing_slash_does_not_redirect() {
    assert!(!should_redirect("/api/bundle.js/"));
}

#[test]
fn css_asset_trailing_slash_does_not_redirect() {
    assert!(!should_redirect("/api/styles.css/"));
}

#[test]
fn map_file_trailing_slash_does_not_redirect() {
    assert!(!should_redirect("/api/app.map/"));
}

#[test]
fn png_trailing_slash_does_not_redirect() {
    assert!(!should_redirect("/api/logo.png/"));
}

#[test]
fn jpg_trailing_slash_does_not_redirect() {
    assert!(!should_redirect("/api/photo.jpg/"));
}

#[test]
fn svg_trailing_slash_does_not_redirect() {
    assert!(!should_redirect("/api/icon.svg/"));
}

#[test]
fn ico_trailing_slash_does_not_redirect() {
    assert!(!should_redirect("/api/favicon.ico/"));
}

#[test]
fn non_api_path_with_trailing_slash_does_not_redirect() {
    assert!(!should_redirect("/about/"));
}

#[test]
fn static_path_trailing_slash_does_not_redirect() {
    assert!(!should_redirect("/static/file/"));
}
