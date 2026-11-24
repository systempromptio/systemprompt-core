//! Unit tests for content error (error.rs)
//!
//! Tests cover:
//! - ContentError enum variants and Display impl
//! - Error type conversions (From implementations)

use systemprompt_content::{ContentError, ContentParseError};

#[test]
fn test_content_error_from_sqlx_error_maps_to_repository() {
    let error = ContentError::from(sqlx::Error::RowNotFound);
    assert!(
        matches!(error, ContentError::Repository(_)),
        "sqlx errors must be wrapped as a repository error, got: {error:?}"
    );
}

#[test]
fn test_content_error_database_not_postgres() {
    let error = ContentError::DatabaseNotPostgres;
    assert_eq!(error.to_string(), "database must be PostgreSQL");
}

#[test]
fn test_content_error_validation() {
    let error = ContentError::Validation("Title must not be empty".to_string());
    let msg = error.to_string();
    assert!(msg.contains("validation"));
    assert!(msg.contains("Title must not be empty"));
}

#[test]
fn test_content_error_parse() {
    let error = ContentError::Parse(ContentParseError::AmbiguousTimezone);
    let msg = error.to_string();
    assert_eq!(msg, "parse error: Ambiguous timezone conversion");
}

#[test]
fn test_content_error_from_serde_json() {
    let invalid_json = "{invalid}";
    let json_error = serde_json::from_str::<serde_json::Value>(invalid_json).unwrap_err();
    let content_error: ContentError = json_error.into();

    let msg = content_error.to_string();
    assert!(msg.contains("json"));
}

#[test]
fn test_content_error_from_io() {
    let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "File not found");
    let content_error: ContentError = io_error.into();

    let msg = content_error.to_string();
    assert!(msg.contains("io"));
}

#[test]
fn test_content_error_from_yaml() {
    let invalid_yaml = "key: [unclosed";
    let yaml_error = serde_yaml::from_str::<serde_yaml::Value>(invalid_yaml).unwrap_err();
    let content_error: ContentError = yaml_error.into();

    let msg = content_error.to_string();
    assert!(msg.contains("yaml"));
}

#[test]
fn test_content_error_empty_message() {
    let error = ContentError::Validation(String::new());
    let msg = error.to_string();
    assert_eq!(msg, "validation: ");
}

#[test]
fn test_content_error_long_message() {
    let long_msg = "x".repeat(10000);
    let error = ContentError::Validation(long_msg.clone());
    let msg = error.to_string();
    assert!(msg.len() > 10000);
    assert!(msg.contains(&long_msg));
}

#[test]
fn test_content_error_newlines() {
    let error = ContentError::Validation("Line 1\nLine 2\nLine 3".to_string());
    let msg = error.to_string();
    assert!(msg.contains('\n'));
}

#[test]
fn test_content_error_parse_keeps_its_kind() {
    let error = ContentError::Parse(ContentParseError::MissingFrontmatterDelimiters);

    match error {
        ContentError::Parse(ContentParseError::MissingFrontmatterDelimiters) => {},
        other => panic!("Expected a frontmatter parse error, got {other:?}"),
    }
}
