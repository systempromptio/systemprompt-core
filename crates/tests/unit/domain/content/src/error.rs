//! Unit tests for content error (error.rs)
//!
//! Tests cover:
//! - ContentError enum variants and Display impl
//! - Error type conversions (From implementations)

use systemprompt_core_content::ContentError;

// ============================================================================
// ContentError Display Tests
// ============================================================================

#[test]
fn test_content_error_database_not_postgres() {
    let error = ContentError::DatabaseNotPostgres;
    assert_eq!(error.to_string(), "Database must be PostgreSQL");
}

#[test]
fn test_content_error_content_not_found() {
    let error = ContentError::ContentNotFound("article-123".to_string());
    let msg = error.to_string();
    assert!(msg.contains("Content not found"));
    assert!(msg.contains("article-123"));
}

#[test]
fn test_content_error_link_not_found() {
    let error = ContentError::LinkNotFound("link-456".to_string());
    let msg = error.to_string();
    assert!(msg.contains("Link not found"));
    assert!(msg.contains("link-456"));
}

#[test]
fn test_content_error_invalid_request() {
    let error = ContentError::InvalidRequest("Missing required field".to_string());
    let msg = error.to_string();
    assert!(msg.contains("Invalid request"));
    assert!(msg.contains("Missing required field"));
}

#[test]
fn test_content_error_validation() {
    let error = ContentError::Validation("Title must not be empty".to_string());
    let msg = error.to_string();
    assert!(msg.contains("Validation error"));
    assert!(msg.contains("Title must not be empty"));
}

#[test]
fn test_content_error_parse() {
    let error = ContentError::Parse("Invalid date format".to_string());
    let msg = error.to_string();
    assert!(msg.contains("Parse error"));
    assert!(msg.contains("Invalid date format"));
}

// ============================================================================
// ContentError Debug Tests
// ============================================================================

#[test]
fn test_content_error_debug_database_not_postgres() {
    let error = ContentError::DatabaseNotPostgres;
    let debug = format!("{:?}", error);
    assert!(debug.contains("DatabaseNotPostgres"));
}

#[test]
fn test_content_error_debug_content_not_found() {
    let error = ContentError::ContentNotFound("test".to_string());
    let debug = format!("{:?}", error);
    assert!(debug.contains("ContentNotFound"));
    assert!(debug.contains("test"));
}

#[test]
fn test_content_error_debug_validation() {
    let error = ContentError::Validation("validation message".to_string());
    let debug = format!("{:?}", error);
    assert!(debug.contains("Validation"));
}

// ============================================================================
// ContentError Serialization From Conversions
// ============================================================================

#[test]
fn test_content_error_from_serde_json() {
    let invalid_json = "{invalid}";
    let json_error = serde_json::from_str::<serde_json::Value>(invalid_json).unwrap_err();
    let content_error: ContentError = json_error.into();

    let msg = content_error.to_string();
    assert!(msg.contains("Serialization error"));
}

#[test]
fn test_content_error_from_io() {
    let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "File not found");
    let content_error: ContentError = io_error.into();

    let msg = content_error.to_string();
    assert!(msg.contains("IO error"));
}

#[test]
fn test_content_error_from_yaml() {
    let invalid_yaml = "key: [unclosed";
    let yaml_error = serde_yaml::from_str::<serde_yaml::Value>(invalid_yaml).unwrap_err();
    let content_error: ContentError = yaml_error.into();

    let msg = content_error.to_string();
    assert!(msg.contains("YAML parse error"));
}

// ============================================================================
// ContentError Edge Cases
// ============================================================================

#[test]
fn test_content_error_empty_message() {
    let error = ContentError::Validation(String::new());
    let msg = error.to_string();
    assert_eq!(msg, "Validation error: ");
}

#[test]
fn test_content_error_long_message() {
    let long_msg = "x".repeat(10000);
    let error = ContentError::Parse(long_msg.clone());
    let msg = error.to_string();
    assert!(msg.len() > 10000);
    assert!(msg.contains(&long_msg));
}

#[test]
fn test_content_error_special_characters() {
    let error = ContentError::ContentNotFound("article-with-special-chars-!@#$%".to_string());
    let msg = error.to_string();
    assert!(msg.contains("!@#$%"));
}

#[test]
fn test_content_error_unicode() {
    let error = ContentError::InvalidRequest("Unicode: ".to_string());
    let msg = error.to_string();
    assert!(msg.contains(""));
}

#[test]
fn test_content_error_newlines() {
    let error = ContentError::Validation("Line 1\nLine 2\nLine 3".to_string());
    let msg = error.to_string();
    assert!(msg.contains('\n'));
}

// ============================================================================
// ContentError Pattern Tests
// ============================================================================

#[test]
fn test_content_error_matching() {
    let error = ContentError::ContentNotFound("test".to_string());

    match error {
        ContentError::ContentNotFound(id) => assert_eq!(id, "test"),
        _ => panic!("Expected ContentNotFound variant"),
    }
}

#[test]
fn test_content_error_is_not_found() {
    fn is_not_found(error: &ContentError) -> bool {
        matches!(
            error,
            ContentError::ContentNotFound(_) | ContentError::LinkNotFound(_)
        )
    }

    assert!(is_not_found(&ContentError::ContentNotFound("a".to_string())));
    assert!(is_not_found(&ContentError::LinkNotFound("b".to_string())));
    assert!(!is_not_found(&ContentError::DatabaseNotPostgres));
    assert!(!is_not_found(&ContentError::Validation("c".to_string())));
}
