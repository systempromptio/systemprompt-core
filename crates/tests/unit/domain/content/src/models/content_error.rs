//! Unit tests for content error model (models/content_error.rs)
//!
//! Tests cover:
//! - ContentError enum variants
//! - Error factory methods

use systemprompt_core_content::models::ContentError;

// ============================================================================
// ContentError Variant Tests
// ============================================================================

#[test]
fn test_content_error_missing_field() {
    let error = ContentError::missing_field("title");
    let msg = error.to_string();
    assert!(msg.contains("Missing required field"));
    assert!(msg.contains("title"));
}

#[test]
fn test_content_error_missing_field_from_string() {
    let error = ContentError::missing_field("description".to_string());
    let msg = error.to_string();
    assert!(msg.contains("description"));
}

#[test]
fn test_content_error_missing_org_config() {
    let error = ContentError::missing_org_config("org_name");
    let msg = error.to_string();
    assert!(msg.contains("Missing organization config"));
    assert!(msg.contains("org_name"));
}

#[test]
fn test_content_error_missing_article_config() {
    let error = ContentError::missing_article_config("author");
    let msg = error.to_string();
    assert!(msg.contains("Missing article config"));
    assert!(msg.contains("author"));
}

#[test]
fn test_content_error_invalid_content() {
    let error = ContentError::invalid_content("Invalid format detected");
    let msg = error.to_string();
    assert!(msg.contains("Invalid content"));
    assert!(msg.contains("Invalid format detected"));
}

#[test]
fn test_content_error_missing_branding_config() {
    let error = ContentError::missing_branding_config("logo");
    let msg = error.to_string();
    assert!(msg.contains("Missing branding config"));
    assert!(msg.contains("logo"));
}

// ============================================================================
// ContentError Display Tests
// ============================================================================

#[test]
fn test_content_error_display_missing_field() {
    let error = ContentError::MissingField {
        field: "slug".to_string(),
    };
    assert_eq!(format!("{}", error), "Missing required field: slug");
}

#[test]
fn test_content_error_display_missing_org_config() {
    let error = ContentError::MissingOrgConfig {
        field: "api_key".to_string(),
    };
    assert!(format!("{}", error).contains("Missing organization config"));
}

#[test]
fn test_content_error_display_missing_article_config() {
    let error = ContentError::MissingArticleConfig {
        field: "publish_date".to_string(),
    };
    assert!(format!("{}", error).contains("Missing article config"));
}

#[test]
fn test_content_error_display_invalid_content() {
    let error = ContentError::InvalidContent {
        message: "Content too short".to_string(),
    };
    assert_eq!(format!("{}", error), "Invalid content: Content too short");
}

#[test]
fn test_content_error_display_missing_branding_config() {
    let error = ContentError::MissingBrandingConfig {
        field: "primary_color".to_string(),
    };
    assert!(format!("{}", error).contains("Missing branding config"));
}

// ============================================================================
// ContentError Debug Tests
// ============================================================================

#[test]
fn test_content_error_debug() {
    let error = ContentError::missing_field("test");
    let debug = format!("{:?}", error);
    assert!(debug.contains("MissingField"));
    assert!(debug.contains("test"));
}

// ============================================================================
// ContentError Factory Methods Edge Cases
// ============================================================================

#[test]
fn test_content_error_empty_field_name() {
    let error = ContentError::missing_field("");
    let msg = error.to_string();
    assert!(msg.contains("Missing required field: "));
}

#[test]
fn test_content_error_long_message() {
    let long_message = "A".repeat(1000);
    let error = ContentError::invalid_content(&long_message);
    let msg = error.to_string();
    assert!(msg.len() > 1000);
}

#[test]
fn test_content_error_special_characters() {
    let error = ContentError::missing_field("field_with_special_chars!@#$%");
    let msg = error.to_string();
    assert!(msg.contains("field_with_special_chars!@#$%"));
}

#[test]
fn test_content_error_unicode() {
    let error = ContentError::invalid_content("Error with unicode: ");
    let msg = error.to_string();
    assert!(msg.contains(""));
}
