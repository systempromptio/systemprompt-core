//! Unit tests for content builder patterns
//!
//! Tests cover:
//! - CreateContentParams builder
//! - UpdateContentParams builder

use chrono::{TimeZone, Utc};
use systemprompt_core_content::models::{ContentKind, CreateContentParams, UpdateContentParams};
use systemprompt_identifiers::{CategoryId, ContentId, SourceId};

// ============================================================================
// CreateContentParams Tests
// ============================================================================

#[test]
fn test_create_content_params_new() {
    let params = CreateContentParams::new(
        "test-slug".to_string(),
        "Test Title".to_string(),
        "Test Description".to_string(),
        "Test body content".to_string(),
        SourceId::new("blog"),
    );

    assert_eq!(params.slug, "test-slug");
    assert_eq!(params.title, "Test Title");
    assert_eq!(params.description, "Test Description");
    assert_eq!(params.body, "Test body content");
    assert_eq!(params.source_id.as_str(), "blog");
    assert!(params.author.is_empty());
    assert!(params.keywords.is_empty());
    assert_eq!(params.kind, ContentKind::Article);
    assert!(params.image.is_none());
    assert!(params.category_id.is_none());
    assert!(params.version_hash.is_empty());
}

#[test]
fn test_create_content_params_with_author() {
    let params = CreateContentParams::new(
        "slug".to_string(),
        "Title".to_string(),
        "Desc".to_string(),
        "Body".to_string(),
        SourceId::new("src"),
    )
    .with_author("John Doe".to_string());

    assert_eq!(params.author, "John Doe");
}

#[test]
fn test_create_content_params_with_published_at() {
    let date = Utc.with_ymd_and_hms(2024, 6, 15, 12, 0, 0).unwrap();
    let params = CreateContentParams::new(
        "slug".to_string(),
        "Title".to_string(),
        "Desc".to_string(),
        "Body".to_string(),
        SourceId::new("src"),
    )
    .with_published_at(date);

    assert_eq!(params.published_at, date);
}

#[test]
fn test_create_content_params_with_keywords() {
    let params = CreateContentParams::new(
        "slug".to_string(),
        "Title".to_string(),
        "Desc".to_string(),
        "Body".to_string(),
        SourceId::new("src"),
    )
    .with_keywords("rust, programming, tutorial".to_string());

    assert_eq!(params.keywords, "rust, programming, tutorial");
}

#[test]
fn test_create_content_params_with_kind() {
    let params = CreateContentParams::new(
        "slug".to_string(),
        "Title".to_string(),
        "Desc".to_string(),
        "Body".to_string(),
        SourceId::new("src"),
    )
    .with_kind(ContentKind::Paper);

    assert_eq!(params.kind, ContentKind::Paper);
}

#[test]
fn test_create_content_params_with_image() {
    let params = CreateContentParams::new(
        "slug".to_string(),
        "Title".to_string(),
        "Desc".to_string(),
        "Body".to_string(),
        SourceId::new("src"),
    )
    .with_image(Some("/images/cover.png".to_string()));

    assert_eq!(params.image, Some("/images/cover.png".to_string()));
}

#[test]
fn test_create_content_params_with_image_none() {
    let params = CreateContentParams::new(
        "slug".to_string(),
        "Title".to_string(),
        "Desc".to_string(),
        "Body".to_string(),
        SourceId::new("src"),
    )
    .with_image(None);

    assert!(params.image.is_none());
}

#[test]
fn test_create_content_params_with_category_id() {
    let params = CreateContentParams::new(
        "slug".to_string(),
        "Title".to_string(),
        "Desc".to_string(),
        "Body".to_string(),
        SourceId::new("src"),
    )
    .with_category_id(Some(CategoryId::new("programming")));

    assert_eq!(params.category_id.as_ref().unwrap().as_str(), "programming");
}

#[test]
fn test_create_content_params_with_version_hash() {
    let params = CreateContentParams::new(
        "slug".to_string(),
        "Title".to_string(),
        "Desc".to_string(),
        "Body".to_string(),
        SourceId::new("src"),
    )
    .with_version_hash("abc123hash".to_string());

    assert_eq!(params.version_hash, "abc123hash");
}

#[test]
fn test_create_content_params_with_links() {
    let links = serde_json::json!([
        {"title": "Link 1", "url": "https://example.com/1"},
        {"title": "Link 2", "url": "https://example.com/2"}
    ]);

    let params = CreateContentParams::new(
        "slug".to_string(),
        "Title".to_string(),
        "Desc".to_string(),
        "Body".to_string(),
        SourceId::new("src"),
    )
    .with_links(links.clone());

    assert_eq!(params.links, links);
}

#[test]
fn test_create_content_params_builder_chain() {
    let date = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
    let params = CreateContentParams::new(
        "complete-article".to_string(),
        "Complete Article".to_string(),
        "A complete article".to_string(),
        "Full body content here".to_string(),
        SourceId::new("blog"),
    )
    .with_author("Jane Smith".to_string())
    .with_published_at(date)
    .with_keywords("complete, test".to_string())
    .with_kind(ContentKind::Guide)
    .with_image(Some("/img/guide.png".to_string()))
    .with_category_id(Some(CategoryId::new("guides")))
    .with_version_hash("fullhash123".to_string())
    .with_links(serde_json::json!([]));

    assert_eq!(params.slug, "complete-article");
    assert_eq!(params.author, "Jane Smith");
    assert_eq!(params.published_at, date);
    assert_eq!(params.keywords, "complete, test");
    assert_eq!(params.kind, ContentKind::Guide);
    assert_eq!(params.image, Some("/img/guide.png".to_string()));
    assert_eq!(params.category_id.as_ref().unwrap().as_str(), "guides");
    assert_eq!(params.version_hash, "fullhash123");
}

#[test]
fn test_create_content_params_clone() {
    let params = CreateContentParams::new(
        "clone".to_string(),
        "Clone".to_string(),
        "Desc".to_string(),
        "Body".to_string(),
        SourceId::new("src"),
    )
    .with_author("Author".to_string());

    let cloned = params.clone();
    assert_eq!(cloned.slug, params.slug);
    assert_eq!(cloned.author, params.author);
}

// ============================================================================
// UpdateContentParams Tests
// ============================================================================

#[test]
fn test_update_content_params_new() {
    let id = ContentId::new("content-123");
    let params = UpdateContentParams::new(
        id.clone(),
        "Updated Title".to_string(),
        "Updated Description".to_string(),
        "Updated body content".to_string(),
    );

    assert_eq!(params.id.as_str(), "content-123");
    assert_eq!(params.title, "Updated Title");
    assert_eq!(params.description, "Updated Description");
    assert_eq!(params.body, "Updated body content");
    assert!(params.keywords.is_empty());
    assert!(params.image.is_none());
    assert!(params.version_hash.is_empty());
}

#[test]
fn test_update_content_params_with_keywords() {
    let params = UpdateContentParams::new(
        ContentId::new("id"),
        "Title".to_string(),
        "Desc".to_string(),
        "Body".to_string(),
    )
    .with_keywords("updated, keywords".to_string());

    assert_eq!(params.keywords, "updated, keywords");
}

#[test]
fn test_update_content_params_with_image() {
    let params = UpdateContentParams::new(
        ContentId::new("id"),
        "Title".to_string(),
        "Desc".to_string(),
        "Body".to_string(),
    )
    .with_image(Some("/new/image.png".to_string()));

    assert_eq!(params.image, Some("/new/image.png".to_string()));
}

#[test]
fn test_update_content_params_with_version_hash() {
    let params = UpdateContentParams::new(
        ContentId::new("id"),
        "Title".to_string(),
        "Desc".to_string(),
        "Body".to_string(),
    )
    .with_version_hash("newhash456".to_string());

    assert_eq!(params.version_hash, "newhash456");
}

#[test]
fn test_update_content_params_builder_chain() {
    let params = UpdateContentParams::new(
        ContentId::new("update-id"),
        "Full Update".to_string(),
        "Full description".to_string(),
        "Full body".to_string(),
    )
    .with_keywords("full, update".to_string())
    .with_image(Some("/full/img.png".to_string()))
    .with_version_hash("fullhash".to_string());

    assert_eq!(params.id.as_str(), "update-id");
    assert_eq!(params.keywords, "full, update");
    assert_eq!(params.image, Some("/full/img.png".to_string()));
    assert_eq!(params.version_hash, "fullhash");
}

#[test]
fn test_update_content_params_clone() {
    let params = UpdateContentParams::new(
        ContentId::new("clone-id"),
        "Clone Title".to_string(),
        "Clone Desc".to_string(),
        "Clone Body".to_string(),
    )
    .with_keywords("clone".to_string());

    let cloned = params.clone();
    assert_eq!(cloned.id.as_str(), params.id.as_str());
    assert_eq!(cloned.title, params.title);
    assert_eq!(cloned.keywords, params.keywords);
}
