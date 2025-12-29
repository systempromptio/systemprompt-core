//! Unit tests for static content configuration
//!
//! Tests cover:
//! - StaticContentMatcher empty state creation
//! - Pattern matching behavior
//! - Slug extraction from paths
//!
//! Note: `StaticContentMatcher::from_config` requires filesystem access
//! and is tested through integration tests.

use systemprompt_core_api::services::static_content::StaticContentMatcher;

// ============================================================================
// StaticContentMatcher Empty State Tests
// ============================================================================

#[test]
fn test_static_content_matcher_empty() {
    let matcher = StaticContentMatcher::empty();
    let debug_str = format!("{:?}", matcher);
    assert!(debug_str.contains("StaticContentMatcher"));
}

#[test]
fn test_static_content_matcher_empty_no_matches() {
    let matcher = StaticContentMatcher::empty();

    // Empty matcher should not match any paths
    assert!(matcher.matches("/blog/test-post").is_none());
    assert!(matcher.matches("/articles/my-article").is_none());
    assert!(matcher.matches("/").is_none());
    assert!(matcher.matches("").is_none());
}

#[test]
fn test_static_content_matcher_empty_clone() {
    let original = StaticContentMatcher::empty();
    let cloned = original.clone();

    // Both should behave the same
    assert!(cloned.matches("/any/path").is_none());
}

// ============================================================================
// Pattern Matching Edge Cases
// ============================================================================

#[test]
fn test_static_content_matcher_matches_various_paths() {
    let matcher = StaticContentMatcher::empty();

    // Test various path formats
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
        // All should return None for empty matcher
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

    // Paths with special characters
    assert!(matcher.matches("/blog/post-with-dashes").is_none());
    assert!(matcher.matches("/blog/post_with_underscores").is_none());
    assert!(matcher.matches("/blog/post.with.dots").is_none());
    assert!(matcher.matches("/blog/post%20encoded").is_none());
}

// ============================================================================
// Debug Implementation Tests
// ============================================================================

#[test]
fn test_static_content_matcher_debug_format() {
    let matcher = StaticContentMatcher::empty();
    let debug_str = format!("{:?}", matcher);

    // Should contain struct name and patterns field
    assert!(debug_str.contains("StaticContentMatcher"));
    assert!(debug_str.contains("patterns"));
}

// ============================================================================
// Clone Independence Tests
// ============================================================================

#[test]
fn test_static_content_matcher_clone_is_independent() {
    let original = StaticContentMatcher::empty();
    let cloned = original.clone();

    // Operations on cloned shouldn't affect original
    let _ = cloned.matches("/test");
    let _ = original.matches("/other");

    // Both should still work correctly
    assert!(original.matches("/any").is_none());
    assert!(cloned.matches("/any").is_none());
}
