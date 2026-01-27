//! Unit tests for validation services
//!
//! Tests cover:
//! - validate_content_metadata
//! - is_valid_date_format (internal function behavior)

use systemprompt_content::{validate_content_metadata, ContentMetadata};

fn create_valid_metadata() -> ContentMetadata {
    ContentMetadata {
        title: "Valid Title".to_string(),
        description: "Valid description".to_string(),
        author: "John Doe".to_string(),
        published_at: "2024-01-15".to_string(),
        slug: "valid-slug".to_string(),
        keywords: "test, valid".to_string(),
        kind: "article".to_string(),
        image: None,
        category: None,
        tags: vec![],
        links: vec![],
    }
}

#[test]
fn test_validate_content_metadata_valid() {
    let metadata = create_valid_metadata();
    let result = validate_content_metadata(&metadata);
    assert!(result.is_ok());
}

#[test]
fn test_validate_content_metadata_empty_title() {
    let mut metadata = create_valid_metadata();
    metadata.title = "".to_string();

    let result = validate_content_metadata(&metadata);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("title"));
}

#[test]
fn test_validate_content_metadata_whitespace_title() {
    let mut metadata = create_valid_metadata();
    metadata.title = "   ".to_string();

    let result = validate_content_metadata(&metadata);
    assert!(result.is_err());
}

#[test]
fn test_validate_content_metadata_empty_slug_allowed_for_index() {
    let mut metadata = create_valid_metadata();
    metadata.slug = "".to_string();

    let result = validate_content_metadata(&metadata);
    assert!(result.is_ok());
}

#[test]
fn test_validate_content_metadata_empty_author() {
    let mut metadata = create_valid_metadata();
    metadata.author = "".to_string();

    let result = validate_content_metadata(&metadata);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("author"));
}

#[test]
fn test_validate_content_metadata_empty_published_at() {
    let mut metadata = create_valid_metadata();
    metadata.published_at = "".to_string();

    let result = validate_content_metadata(&metadata);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("published_at"));
}

#[test]
fn test_validate_content_metadata_invalid_slug_uppercase() {
    let mut metadata = create_valid_metadata();
    metadata.slug = "Invalid-Slug".to_string();

    let result = validate_content_metadata(&metadata);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("slug"));
}

#[test]
fn test_validate_content_metadata_invalid_slug_spaces() {
    let mut metadata = create_valid_metadata();
    metadata.slug = "invalid slug".to_string();

    let result = validate_content_metadata(&metadata);
    assert!(result.is_err());
}

#[test]
fn test_validate_content_metadata_valid_slug_with_numbers() {
    let mut metadata = create_valid_metadata();
    metadata.slug = "article-2024-01".to_string();

    let result = validate_content_metadata(&metadata);
    assert!(result.is_ok());
}

#[test]
fn test_validate_content_metadata_invalid_date_format() {
    let mut metadata = create_valid_metadata();
    metadata.published_at = "01-15-2024".to_string();

    let result = validate_content_metadata(&metadata);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("YYYY-MM-DD"));
}

#[test]
fn test_validate_content_metadata_invalid_date_short() {
    let mut metadata = create_valid_metadata();
    metadata.published_at = "2024-1-5".to_string();

    let result = validate_content_metadata(&metadata);
    assert!(result.is_err());
}

#[test]
fn test_validate_content_metadata_nested_slug_simple() {
    let mut metadata = create_valid_metadata();
    metadata.slug = "getting-started/installation".to_string();

    let result = validate_content_metadata(&metadata);
    assert!(result.is_ok());
}

#[test]
fn test_validate_content_metadata_nested_slug_deep() {
    let mut metadata = create_valid_metadata();
    metadata.slug = "crates/infrastructure/database".to_string();

    let result = validate_content_metadata(&metadata);
    assert!(result.is_ok());
}

#[test]
fn test_validate_content_metadata_nested_slug_with_numbers() {
    let mut metadata = create_valid_metadata();
    metadata.slug = "guides/v2/migration-2024".to_string();

    let result = validate_content_metadata(&metadata);
    assert!(result.is_ok());
}

#[test]
fn test_validate_content_metadata_nested_slug_rejects_double_slash() {
    let mut metadata = create_valid_metadata();
    metadata.slug = "guides//migration".to_string();

    let result = validate_content_metadata(&metadata);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("double slashes"));
}

#[test]
fn test_validate_content_metadata_nested_slug_rejects_uppercase_segment() {
    let mut metadata = create_valid_metadata();
    metadata.slug = "guides/Migration/steps".to_string();

    let result = validate_content_metadata(&metadata);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("slug"));
}

#[test]
fn test_validate_content_metadata_nested_slug_rejects_spaces_in_segment() {
    let mut metadata = create_valid_metadata();
    metadata.slug = "guides/my guide/steps".to_string();

    let result = validate_content_metadata(&metadata);
    assert!(result.is_err());
}

#[test]
fn test_validate_content_metadata_nested_slug_allows_leading_trailing_slash_trim() {
    let mut metadata = create_valid_metadata();
    metadata.slug = "/guides/migration/".to_string();

    let result = validate_content_metadata(&metadata);
    assert!(result.is_ok());
}

#[test]
fn test_validate_content_metadata_slug_only_slashes() {
    let mut metadata = create_valid_metadata();
    metadata.slug = "///".to_string();

    let result = validate_content_metadata(&metadata);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("only slashes"));
}

#[test]
fn test_valid_date_formats() {
    let valid_dates = vec![
        "2024-01-01",
        "2024-12-31",
        "2000-06-15",
        "1999-01-01",
        "2099-12-31",
    ];

    for date in valid_dates {
        let mut metadata = create_valid_metadata();
        metadata.published_at = date.to_string();

        let result = validate_content_metadata(&metadata);
        assert!(result.is_ok(), "Date '{}' should be valid", date);
    }
}

#[test]
fn test_invalid_date_formats() {
    let invalid_dates = vec![
        "2024/01/01",
        "24-01-01",
        "2024-1-01",
        "2024-01-1",
        "01-01-2024",
        "not-a-date",
        "2024",
        "2024-01",
    ];

    for date in invalid_dates {
        let mut metadata = create_valid_metadata();
        metadata.published_at = date.to_string();

        let result = validate_content_metadata(&metadata);
        assert!(result.is_err(), "Date '{}' should be invalid", date);
    }
}
