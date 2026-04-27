//! Unit tests for SessionMiddleware::should_skip_session_tracking
//!
//! Tests cover:
//! - API paths are skipped
//! - MCP paths are skipped
//! - Static asset paths are skipped
//! - Health/ready endpoints are skipped
//! - Well-known static files are skipped
//! - Track paths are NOT skipped
//! - Content page paths are NOT skipped

use systemprompt_api::services::middleware::SessionMiddleware;
use systemprompt_models::modules::ApiPaths;

#[test]
fn api_base_is_skipped() {
    assert!(SessionMiddleware::should_skip_session_tracking(
        ApiPaths::API_BASE
    ));
}

#[test]
fn api_v1_is_skipped() {
    assert!(SessionMiddleware::should_skip_session_tracking(
        ApiPaths::API_V1
    ));
}

#[test]
fn mcp_base_is_skipped() {
    assert!(SessionMiddleware::should_skip_session_tracking(
        ApiPaths::MCP_BASE
    ));
}

#[test]
fn next_base_is_skipped() {
    assert!(SessionMiddleware::should_skip_session_tracking(
        ApiPaths::NEXT_BASE
    ));
}

#[test]
fn static_base_is_skipped() {
    assert!(SessionMiddleware::should_skip_session_tracking(
        ApiPaths::STATIC_BASE
    ));
}

#[test]
fn assets_base_is_skipped() {
    assert!(SessionMiddleware::should_skip_session_tracking(
        ApiPaths::ASSETS_BASE
    ));
}

#[test]
fn images_base_is_skipped() {
    assert!(SessionMiddleware::should_skip_session_tracking(
        ApiPaths::IMAGES_BASE
    ));
}

#[test]
fn health_endpoint_is_skipped() {
    assert!(SessionMiddleware::should_skip_session_tracking("/health"));
}

#[test]
fn ready_endpoint_is_skipped() {
    assert!(SessionMiddleware::should_skip_session_tracking("/ready"));
}

#[test]
fn healthz_endpoint_is_skipped() {
    assert!(SessionMiddleware::should_skip_session_tracking("/healthz"));
}

#[test]
fn favicon_is_skipped() {
    assert!(SessionMiddleware::should_skip_session_tracking(
        "/favicon.ico"
    ));
}

#[test]
fn robots_txt_is_skipped() {
    assert!(SessionMiddleware::should_skip_session_tracking(
        "/robots.txt"
    ));
}

#[test]
fn sitemap_xml_is_skipped() {
    assert!(SessionMiddleware::should_skip_session_tracking(
        "/sitemap.xml"
    ));
}

#[test]
fn manifest_json_is_skipped() {
    assert!(SessionMiddleware::should_skip_session_tracking(
        "/manifest.json"
    ));
}

#[test]
fn css_file_is_skipped() {
    assert!(SessionMiddleware::should_skip_session_tracking(
        "/styles/main.css"
    ));
}

#[test]
fn js_file_is_skipped() {
    assert!(SessionMiddleware::should_skip_session_tracking(
        "/scripts/app.js"
    ));
}

#[test]
fn png_file_is_skipped() {
    assert!(SessionMiddleware::should_skip_session_tracking(
        "/images/logo.png"
    ));
}

#[test]
fn track_base_is_not_skipped() {
    assert!(!SessionMiddleware::should_skip_session_tracking(
        ApiPaths::TRACK_BASE
    ));
}

#[test]
fn track_engagement_is_not_skipped() {
    assert!(!SessionMiddleware::should_skip_session_tracking(
        ApiPaths::TRACK_ENGAGEMENT
    ));
}

#[test]
fn content_page_is_not_skipped() {
    assert!(!SessionMiddleware::should_skip_session_tracking("/about"));
}

#[test]
fn root_path_is_not_skipped() {
    assert!(!SessionMiddleware::should_skip_session_tracking("/"));
}

#[test]
fn html_page_is_not_skipped() {
    assert!(!SessionMiddleware::should_skip_session_tracking(
        "/page.html"
    ));
}

#[test]
fn nested_content_path_is_not_skipped() {
    assert!(!SessionMiddleware::should_skip_session_tracking(
        "/blog/my-post"
    ));
}
