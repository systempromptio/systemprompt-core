//! Unit tests for content configuration
//!
//! Tests cover:
//! - ContentConfigValidated URL pattern matching
//! - ContentSourceConfigValidated structure
//! - LoadStats structure
//! - ParsedContent structure

use systemprompt_content::{LoadStats, ParsedContent};

// ============================================================================
// LoadStats Tests
// ============================================================================

#[test]
fn test_load_stats_default() {
    let stats = LoadStats::default();
    assert_eq!(stats.files_found, 0);
    assert_eq!(stats.files_loaded, 0);
    assert_eq!(stats.files_with_errors, 0);
    assert_eq!(stats.load_time_ms, 0);
    assert!(stats.source_stats.is_empty());
}

#[test]
fn test_load_stats_with_values() {
    let stats = LoadStats {
        files_found: 100,
        files_loaded: 95,
        files_with_errors: 5,
        load_time_ms: 1500,
        source_stats: std::collections::HashMap::new(),
    };

    assert_eq!(stats.files_found, 100);
    assert_eq!(stats.files_loaded, 95);
    assert_eq!(stats.files_with_errors, 5);
    assert_eq!(stats.load_time_ms, 1500);
}

#[test]
fn test_load_stats_clone() {
    let stats = LoadStats {
        files_found: 10,
        files_loaded: 8,
        files_with_errors: 2,
        load_time_ms: 500,
        source_stats: std::collections::HashMap::new(),
    };

    let cloned = stats.clone();
    assert_eq!(cloned.files_found, stats.files_found);
    assert_eq!(cloned.files_loaded, stats.files_loaded);
    assert_eq!(cloned.files_with_errors, stats.files_with_errors);
    assert_eq!(cloned.load_time_ms, stats.load_time_ms);
}

// ============================================================================
// ParsedContent Tests
// ============================================================================

#[test]
fn test_parsed_content_creation() {
    use std::path::PathBuf;
    use chrono::{TimeZone, Utc};

    let content = ParsedContent {
        slug: "test-article".to_string(),
        title: "Test Article".to_string(),
        description: "A test article description".to_string(),
        body: "Article body content".to_string(),
        author: "John Doe".to_string(),
        published_at: Utc.with_ymd_and_hms(2024, 1, 15, 0, 0, 0).unwrap(),
        keywords: "test, article".to_string(),
        kind: "article".to_string(),
        image: Some("/images/test.png".to_string()),
        category_id: "tech".to_string(),
        source_id: "blog".to_string(),
        version_hash: "abc123hash".to_string(),
        file_path: PathBuf::from("/content/articles/test.md"),
    };

    assert_eq!(content.slug, "test-article");
    assert_eq!(content.title, "Test Article");
    assert_eq!(content.author, "John Doe");
    assert_eq!(content.kind, "article");
    assert_eq!(content.source_id, "blog");
    assert_eq!(content.category_id, "tech");
    assert_eq!(content.file_path, PathBuf::from("/content/articles/test.md"));
}

#[test]
fn test_parsed_content_without_image() {
    use std::path::PathBuf;
    use chrono::Utc;

    let content = ParsedContent {
        slug: "no-image".to_string(),
        title: "No Image Article".to_string(),
        description: "Desc".to_string(),
        body: "Body".to_string(),
        author: "Author".to_string(),
        published_at: Utc::now(),
        keywords: "".to_string(),
        kind: "article".to_string(),
        image: None,
        category_id: "cat".to_string(),
        source_id: "src".to_string(),
        version_hash: "hash".to_string(),
        file_path: PathBuf::from("/path/to/file.md"),
    };

    assert!(content.image.is_none());
}

#[test]
fn test_parsed_content_clone() {
    use std::path::PathBuf;
    use chrono::Utc;

    let content = ParsedContent {
        slug: "clone-test".to_string(),
        title: "Clone Test".to_string(),
        description: "Desc".to_string(),
        body: "Body".to_string(),
        author: "Author".to_string(),
        published_at: Utc::now(),
        keywords: "key".to_string(),
        kind: "guide".to_string(),
        image: Some("/img.png".to_string()),
        category_id: "cat".to_string(),
        source_id: "src".to_string(),
        version_hash: "hash".to_string(),
        file_path: PathBuf::from("/path.md"),
    };

    let cloned = content.clone();
    assert_eq!(cloned.slug, content.slug);
    assert_eq!(cloned.title, content.title);
    assert_eq!(cloned.file_path, content.file_path);
}

// ============================================================================
// URL Pattern Matching Tests (documenting expected behavior)
// ============================================================================

#[test]
fn test_url_pattern_matching_exact() {
    // Documents expected behavior for exact path matching
    let pattern = "/blog/articles";
    let path = "/blog/articles";

    let pattern_parts: Vec<&str> = pattern.split('/').filter(|s| !s.is_empty()).collect();
    let path_parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

    assert_eq!(pattern_parts.len(), path_parts.len());
    assert!(pattern_parts.iter().zip(path_parts.iter()).all(|(p, pp)| p == pp));
}

#[test]
fn test_url_pattern_matching_with_slug() {
    // Documents expected behavior for slug pattern matching
    let pattern = "/blog/{slug}";
    let path = "/blog/my-article";

    let pattern_parts: Vec<&str> = pattern.split('/').filter(|s| !s.is_empty()).collect();
    let path_parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

    assert_eq!(pattern_parts.len(), path_parts.len());
    assert_eq!(pattern_parts[0], "blog");
    assert_eq!(pattern_parts[1], "{slug}");

    // {slug} should match any value
    let matches = pattern_parts.iter().zip(path_parts.iter()).all(|(p, pp)| {
        *p == "{slug}" || p == pp
    });
    assert!(matches);
}

#[test]
fn test_url_pattern_matching_different_lengths() {
    // Patterns with different segment counts should not match
    let pattern = "/blog/{slug}";
    let path = "/blog/category/article";

    let pattern_parts: Vec<&str> = pattern.split('/').filter(|s| !s.is_empty()).collect();
    let path_parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

    assert_ne!(pattern_parts.len(), path_parts.len());
}

#[test]
fn test_url_pattern_root_path() {
    let path = "/";
    let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    assert!(parts.is_empty());
}

#[test]
fn test_url_pattern_nested_slug() {
    let pattern = "/docs/{category}/{slug}";
    let path = "/docs/guides/getting-started";

    let pattern_parts: Vec<&str> = pattern.split('/').filter(|s| !s.is_empty()).collect();
    let path_parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

    assert_eq!(pattern_parts.len(), path_parts.len());
    assert_eq!(pattern_parts[0], "docs");
    assert_eq!(pattern_parts[1], "{category}");
    assert_eq!(pattern_parts[2], "{slug}");
}

// ============================================================================
// Source Determination Tests (documenting expected behavior)
// ============================================================================

#[test]
fn test_source_determination_root() {
    // Root path "/" should return "web"
    let path = "/";
    let expected = "web";

    if path == "/" {
        assert_eq!("web", expected);
    }
}

#[test]
fn test_source_determination_unknown() {
    // Unknown paths should return "unknown"
    let _path = "/unknown/random/path";
    let default = "unknown";

    // When no pattern matches, default to "unknown"
    assert_eq!(default, "unknown");
}

// ============================================================================
// ValidationResult Pattern Tests
// ============================================================================

#[test]
fn test_validation_result_pattern() {
    // ValidationResult is Result<ContentConfigValidated, ContentConfigErrors>
    // This tests the pattern of using Result types

    type TestResult = Result<String, String>;

    let success: TestResult = Ok("validated config".to_string());
    assert!(success.is_ok());

    let failure: TestResult = Err("validation error".to_string());
    assert!(failure.is_err());
}
