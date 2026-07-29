//! Unit tests for static content configuration
//!
//! Tests cover:
//! - StaticContentMatcher empty state creation
//! - Pattern matching behavior
//! - Slug extraction from paths
//!
//! Note: `StaticContentMatcher::from_config` requires filesystem access
//! and is tested through integration tests.

use std::path::Path;
use systemprompt_api::services::static_content::StaticContentMatcher;
use systemprompt_api::services::static_content::static_files::{
    CACHE_HTML, CACHE_METADATA, CACHE_STATIC_ASSET, CACHE_STATIC_ASSET_REVALIDATE,
    asset_cache_policy, compute_etag,
};

#[test]
fn compute_etag_is_deterministic_for_same_input() {
    let a = compute_etag(b"hello world");
    let b = compute_etag(b"hello world");
    assert_eq!(a, b);
}

#[test]
fn compute_etag_differs_for_different_input() {
    let a = compute_etag(b"hello");
    let b = compute_etag(b"world");
    assert_ne!(a, b);
}

#[test]
fn compute_etag_is_wrapped_in_quotes() {
    let a = compute_etag(b"x");
    assert!(a.starts_with('"'));
    assert!(a.ends_with('"'));
}

#[test]
fn cache_constants_have_expected_directives() {
    assert!(CACHE_STATIC_ASSET.contains("public"));
    assert!(CACHE_STATIC_ASSET.contains("immutable"));
    assert_eq!(CACHE_HTML, "no-cache");
    assert!(CACHE_METADATA.contains("max-age"));
    assert_eq!(
        CACHE_STATIC_ASSET_REVALIDATE,
        "public, max-age=0, must-revalidate"
    );
}

#[test]
fn content_hashed_assets_are_served_immutable() {
    for name in [
        "/dist/js/app.4f3a9c1e.js",
        "/dist/css/main-8ba7f21c.css",
        "/dist/js/vendor.0a1b2c3d4e5f6789.min.js",
        "/dist/js/bundle-KJHF3D2A.js",
    ] {
        assert_eq!(
            asset_cache_policy(Path::new(name)),
            CACHE_STATIC_ASSET,
            "{name} should be immutable"
        );
    }
}

#[test]
fn unhashed_assets_are_served_revalidating() {
    for name in [
        "/dist/css/content.css",
        "/dist/js/app.js",
        "/dist/js/app.min.js",
        "/dist/css/v2.css",
        "/dist/images/photo.2024.jpg",
        "/favicon.ico",
        "/dist/fonts/inter-regular.woff2",
    ] {
        assert_eq!(
            asset_cache_policy(Path::new(name)),
            CACHE_STATIC_ASSET_REVALIDATE,
            "{name} should revalidate"
        );
    }
}

#[test]
fn test_static_content_matcher_empty() {
    let matcher = StaticContentMatcher::empty();
    let debug_str = format!("{:?}", matcher);
    assert!(debug_str.contains("StaticContentMatcher"));
}

#[test]
fn test_static_content_matcher_empty_no_matches() {
    let matcher = StaticContentMatcher::empty();

    assert!(matcher.matches("/blog/test-post").is_none());
    assert!(matcher.matches("/articles/my-article").is_none());
    assert!(matcher.matches("/").is_none());
    assert!(matcher.matches("").is_none());
}

#[test]
fn test_static_content_matcher_empty_clone() {
    let original = StaticContentMatcher::empty();
    let cloned = original.clone();

    assert!(cloned.matches("/any/path").is_none());
}

#[test]
fn test_static_content_matcher_matches_various_paths() {
    let matcher = StaticContentMatcher::empty();

    let paths = vec![
        "/",
        "/blog",
        "/blog/",
        "/blog/post-slug",
        "/blog/post-slug/",
        "/articles/2024/01/my-article",
        "/category/subcategory/item",
        "",
        "no-leading-slash",
    ];

    for path in paths {
        assert!(
            matcher.matches(path).is_none(),
            "Path '{}' should not match",
            path
        );
    }
}

#[test]
fn test_static_content_matcher_empty_returns_none_for_special_chars() {
    let matcher = StaticContentMatcher::empty();

    assert!(matcher.matches("/blog/post-with-dashes").is_none());
    assert!(matcher.matches("/blog/post_with_underscores").is_none());
    assert!(matcher.matches("/blog/post.with.dots").is_none());
    assert!(matcher.matches("/blog/post%20encoded").is_none());
}

#[test]
fn test_static_content_matcher_debug_format() {
    let matcher = StaticContentMatcher::empty();
    let debug_str = format!("{:?}", matcher);

    assert!(debug_str.contains("StaticContentMatcher"));
    assert!(debug_str.contains("patterns"));
}

#[test]
fn test_static_content_matcher_clone_is_independent() {
    let original = StaticContentMatcher::empty();
    let cloned = original.clone();

    let _ = cloned.matches("/test");
    let _ = original.matches("/other");

    assert!(original.matches("/any").is_none());
    assert!(cloned.matches("/any").is_none());
}
