//! Unit tests for content models
//!
//! Tests cover:
//! - ContentKind enum (as_str, Display impl)
//! - Content struct (links_metadata deserialization)
//! - IngestionReport (new, is_success, Default impl)
//! - IngestionOptions (builder pattern)
//! - IngestionSource (constructor)

use systemprompt_content::{ContentMetadata, IngestionOptions, IngestionReport, IngestionSource};

// ============================================================================
// ContentKind Tests
// ============================================================================

#[test]
fn test_content_kind_as_str_article() {
    use systemprompt_content::models::ContentKind;
    let kind = ContentKind::Article;
    assert_eq!(kind.as_str(), "article");
}

#[test]
fn test_content_kind_copy() {
    use systemprompt_content::models::ContentKind;
    let kind = ContentKind::Article;
    let copied = kind;
    assert_eq!(copied, ContentKind::Article);
}

#[test]
fn test_content_kind_as_str_guide() {
    use systemprompt_content::models::ContentKind;
    let kind = ContentKind::Guide;
    assert_eq!(kind.as_str(), "guide");
}

#[test]
fn test_content_kind_as_str_tutorial() {
    use systemprompt_content::models::ContentKind;
    let kind = ContentKind::Tutorial;
    assert_eq!(kind.as_str(), "tutorial");
}

#[test]
fn test_content_kind_display() {
    use systemprompt_content::models::ContentKind;
    assert_eq!(format!("{}", ContentKind::Article), "article");
    assert_eq!(format!("{}", ContentKind::Guide), "guide");
    assert_eq!(format!("{}", ContentKind::Tutorial), "tutorial");
}

#[test]
fn test_content_kind_default() {
    use systemprompt_content::models::ContentKind;
    let default_kind = ContentKind::default();
    assert_eq!(default_kind, ContentKind::Article);
}

#[test]
fn test_content_kind_serialization() {
    use systemprompt_content::models::ContentKind;
    let kind = ContentKind::Guide;
    let json = serde_json::to_string(&kind).unwrap();
    assert_eq!(json, "\"guide\"");
}

#[test]
fn test_content_kind_deserialization() {
    use systemprompt_content::models::ContentKind;
    let kind: ContentKind = serde_json::from_str("\"guide\"").unwrap();
    assert_eq!(kind, ContentKind::Guide);
}

// ============================================================================
// IngestionReport Tests
// ============================================================================

#[test]
fn test_ingestion_report_new() {
    let report = IngestionReport::new();
    assert_eq!(report.files_found, 0);
    assert_eq!(report.files_processed, 0);
    assert!(report.errors.is_empty());
}

#[test]
fn test_ingestion_report_default() {
    let report = IngestionReport::default();
    assert_eq!(report.files_found, 0);
    assert_eq!(report.files_processed, 0);
    assert!(report.errors.is_empty());
}

#[test]
fn test_ingestion_report_is_success_empty_errors() {
    let report = IngestionReport::new();
    assert!(report.is_success());
}

#[test]
fn test_ingestion_report_is_success_with_errors() {
    let mut report = IngestionReport::new();
    report.errors.push("Some error".to_string());
    assert!(!report.is_success());
}

#[test]
fn test_ingestion_report_with_data() {
    let mut report = IngestionReport::new();
    report.files_found = 10;
    report.files_processed = 8;
    report.errors.push("File not found".to_string());
    report.errors.push("Parse error".to_string());

    assert_eq!(report.files_found, 10);
    assert_eq!(report.files_processed, 8);
    assert_eq!(report.errors.len(), 2);
    assert!(!report.is_success());
}

// ============================================================================
// IngestionOptions Tests
// ============================================================================

#[test]
fn test_ingestion_options_default() {
    let options = IngestionOptions::default();
    assert!(!options.override_existing);
    assert!(!options.recursive);
}

#[test]
fn test_ingestion_options_with_override() {
    let options = IngestionOptions::default().with_override(true);
    assert!(options.override_existing);
    assert!(!options.recursive);
}

#[test]
fn test_ingestion_options_with_recursive() {
    let options = IngestionOptions::default().with_recursive(true);
    assert!(!options.override_existing);
    assert!(options.recursive);
}

#[test]
fn test_ingestion_options_builder_chain() {
    let options = IngestionOptions::default()
        .with_override(true)
        .with_recursive(true);
    assert!(options.override_existing);
    assert!(options.recursive);
}

#[test]
fn test_ingestion_options_with_override_false() {
    let options = IngestionOptions::default()
        .with_override(true)
        .with_override(false);
    assert!(!options.override_existing);
}

// ============================================================================
// IngestionSource Tests
// ============================================================================

#[test]
fn test_ingestion_source_new() {
    let allowed_types: &[&str] = &["article", "paper"];
    let source = IngestionSource::new("blog", "tech", allowed_types);

    assert_eq!(source.source_id, "blog");
    assert_eq!(source.category_id, "tech");
    assert_eq!(source.allowed_content_types.len(), 2);
    assert_eq!(source.allowed_content_types[0], "article");
    assert_eq!(source.allowed_content_types[1], "paper");
}

#[test]
fn test_ingestion_source_empty_content_types() {
    let allowed_types: &[&str] = &[];
    let source = IngestionSource::new("docs", "documentation", allowed_types);

    assert_eq!(source.source_id, "docs");
    assert_eq!(source.category_id, "documentation");
    assert!(source.allowed_content_types.is_empty());
}

#[test]
fn test_ingestion_source_clone() {
    let allowed_types: &[&str] = &["guide"];
    let source = IngestionSource::new("tutorials", "learning", allowed_types);
    let cloned = source.clone();

    assert_eq!(cloned.source_id, source.source_id);
    assert_eq!(cloned.category_id, source.category_id);
    assert_eq!(cloned.allowed_content_types.len(), source.allowed_content_types.len());
}

// ============================================================================
// ContentMetadata Serialization Tests
// ============================================================================

#[test]
fn test_content_metadata_deserialization_minimal() {
    let yaml = r#"
title: Test Article
slug: test-article
published_at: "2024-01-15"
kind: article
"#;
    let metadata: ContentMetadata = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(metadata.title, "Test Article");
    assert_eq!(metadata.slug, "test-article");
    assert_eq!(metadata.published_at, "2024-01-15");
    assert_eq!(metadata.kind, "article");
    assert!(metadata.description.is_empty());
    assert!(metadata.author.is_empty());
    assert!(metadata.keywords.is_empty());
    assert!(metadata.image.is_none());
    assert!(metadata.category.is_none());
    assert!(metadata.tags.is_empty());
    assert!(metadata.links.is_empty());
}

#[test]
fn test_content_metadata_deserialization_full() {
    let yaml = r#"
title: Complete Guide
description: A comprehensive guide
author: John Doe
published_at: "2024-03-20"
slug: complete-guide
keywords: rust, programming
kind: guide
image: /images/guide.png
category: programming
tags:
  - rust
  - tutorial
links:
  - title: Related Post
    url: https://example.com/related
"#;
    let metadata: ContentMetadata = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(metadata.title, "Complete Guide");
    assert_eq!(metadata.description, "A comprehensive guide");
    assert_eq!(metadata.author, "John Doe");
    assert_eq!(metadata.published_at, "2024-03-20");
    assert_eq!(metadata.slug, "complete-guide");
    assert_eq!(metadata.keywords, "rust, programming");
    assert_eq!(metadata.kind, "guide");
    assert_eq!(metadata.image, Some("/images/guide.png".to_string()));
    assert_eq!(metadata.category, Some("programming".to_string()));
    assert_eq!(metadata.tags.len(), 2);
    assert_eq!(metadata.tags[0], "rust");
    assert_eq!(metadata.tags[1], "tutorial");
    assert_eq!(metadata.links.len(), 1);
    assert_eq!(metadata.links[0].title, "Related Post");
    assert_eq!(metadata.links[0].url, "https://example.com/related");
}

#[test]
fn test_content_metadata_serialization() {
    use systemprompt_content::models::ContentLinkMetadata;

    let metadata = ContentMetadata {
        title: "Test".to_string(),
        description: "Desc".to_string(),
        author: "Author".to_string(),
        published_at: "2024-01-01".to_string(),
        slug: "test".to_string(),
        keywords: "key".to_string(),
        kind: "article".to_string(),
        image: Some("/img.png".to_string()),
        category: Some("cat".to_string()),
        tags: vec!["tag1".to_string()],
        links: vec![ContentLinkMetadata {
            title: "Link".to_string(),
            url: "https://example.com".to_string(),
        }],
    };

    let json = serde_json::to_string(&metadata).unwrap();
    assert!(json.contains("\"title\":\"Test\""));
    assert!(json.contains("\"slug\":\"test\""));
}

// ============================================================================
// ContentLinkMetadata Tests
// ============================================================================

#[test]
fn test_content_link_metadata_creation() {
    use systemprompt_content::models::ContentLinkMetadata;

    let link = ContentLinkMetadata {
        title: "External Resource".to_string(),
        url: "https://example.com/resource".to_string(),
    };

    assert_eq!(link.title, "External Resource");
    assert_eq!(link.url, "https://example.com/resource");
}

#[test]
fn test_content_link_metadata_clone() {
    use systemprompt_content::models::ContentLinkMetadata;

    let link = ContentLinkMetadata {
        title: "Link".to_string(),
        url: "https://example.com".to_string(),
    };
    let cloned = link.clone();

    assert_eq!(cloned.title, link.title);
    assert_eq!(cloned.url, link.url);
}

// ============================================================================
// Content Tests
// ============================================================================

#[test]
fn test_content_links_metadata_valid() {
    use systemprompt_content::models::Content;
    use systemprompt_identifiers::{ContentId, SourceId};
    use chrono::Utc;

    let links_json = serde_json::json!([
        {"title": "Link 1", "url": "https://example.com/1"},
        {"title": "Link 2", "url": "https://example.com/2"}
    ]);

    let content = Content {
        id: ContentId::new("content-1"),
        slug: "test-content".to_string(),
        title: "Test Content".to_string(),
        description: "Description".to_string(),
        body: "Body content".to_string(),
        author: "Author".to_string(),
        published_at: Utc::now(),
        keywords: "test".to_string(),
        kind: "article".to_string(),
        image: None,
        category_id: None,
        source_id: SourceId::new("source"),
        version_hash: "hash".to_string(),
        public: true,
        links: links_json,
        updated_at: Utc::now(),
    };

    let result = content.links_metadata();
    assert!(result.is_ok());
    let links = result.unwrap();
    assert_eq!(links.len(), 2);
    assert_eq!(links[0].title, "Link 1");
    assert_eq!(links[1].url, "https://example.com/2");
}

#[test]
fn test_content_links_metadata_empty() {
    use systemprompt_content::models::Content;
    use systemprompt_identifiers::{ContentId, SourceId};
    use chrono::Utc;

    let content = Content {
        id: ContentId::new("content-2"),
        slug: "no-links".to_string(),
        title: "No Links".to_string(),
        description: "Description".to_string(),
        body: "Body".to_string(),
        author: "Author".to_string(),
        published_at: Utc::now(),
        keywords: "".to_string(),
        kind: "article".to_string(),
        image: None,
        category_id: None,
        source_id: SourceId::new("source"),
        version_hash: "hash".to_string(),
        public: true,
        links: serde_json::json!([]),
        updated_at: Utc::now(),
    };

    let result = content.links_metadata();
    assert!(result.is_ok());
    assert!(result.unwrap().is_empty());
}

#[test]
fn test_content_links_metadata_invalid_json() {
    use systemprompt_content::models::Content;
    use systemprompt_identifiers::{ContentId, SourceId};
    use chrono::Utc;

    let content = Content {
        id: ContentId::new("content-3"),
        slug: "invalid-links".to_string(),
        title: "Invalid Links".to_string(),
        description: "Description".to_string(),
        body: "Body".to_string(),
        author: "Author".to_string(),
        published_at: Utc::now(),
        keywords: "".to_string(),
        kind: "article".to_string(),
        image: None,
        category_id: None,
        source_id: SourceId::new("source"),
        version_hash: "hash".to_string(),
        public: true,
        links: serde_json::json!({"not": "an array"}),
        updated_at: Utc::now(),
    };

    let result = content.links_metadata();
    assert!(result.is_err());
}

#[test]
fn test_content_clone() {
    use systemprompt_content::models::Content;
    use systemprompt_identifiers::{ContentId, SourceId};
    use chrono::Utc;

    let content = Content {
        id: ContentId::new("content-4"),
        slug: "clone-test".to_string(),
        title: "Clone Test".to_string(),
        description: "Description".to_string(),
        body: "Body".to_string(),
        author: "Author".to_string(),
        published_at: Utc::now(),
        keywords: "".to_string(),
        kind: "article".to_string(),
        image: Some("/image.png".to_string()),
        category_id: None,
        source_id: SourceId::new("source"),
        version_hash: "hash".to_string(),
        public: true,
        links: serde_json::json!([]),
        updated_at: Utc::now(),
    };

    let cloned = content.clone();
    assert_eq!(cloned.id, content.id);
    assert_eq!(cloned.slug, content.slug);
    assert_eq!(cloned.title, content.title);
}

// ============================================================================
// ContentSummary Tests
// ============================================================================

#[test]
fn test_content_summary_creation() {
    use systemprompt_content::models::ContentSummary;
    use systemprompt_identifiers::ContentId;
    use chrono::Utc;

    let summary = ContentSummary {
        id: ContentId::new("summary-1"),
        slug: "test-summary".to_string(),
        title: "Test Summary".to_string(),
        description: "Summary description".to_string(),
        published_at: Utc::now(),
    };

    assert_eq!(summary.slug, "test-summary");
    assert_eq!(summary.title, "Test Summary");
}

#[test]
fn test_content_summary_clone() {
    use systemprompt_content::models::ContentSummary;
    use systemprompt_identifiers::ContentId;
    use chrono::Utc;

    let summary = ContentSummary {
        id: ContentId::new("summary-2"),
        slug: "clone-summary".to_string(),
        title: "Clone Summary".to_string(),
        description: "Description".to_string(),
        published_at: Utc::now(),
    };

    let cloned = summary.clone();
    assert_eq!(cloned.id, summary.id);
    assert_eq!(cloned.slug, summary.slug);
}

#[test]
fn test_content_summary_serialization() {
    use systemprompt_content::models::ContentSummary;
    use systemprompt_identifiers::ContentId;
    use chrono::Utc;

    let summary = ContentSummary {
        id: ContentId::new("summary-3"),
        slug: "serial-summary".to_string(),
        title: "Serialization Test".to_string(),
        description: "Description".to_string(),
        published_at: Utc::now(),
    };

    let json = serde_json::to_string(&summary).unwrap();
    assert!(json.contains("\"slug\":\"serial-summary\""));
    assert!(json.contains("\"title\":\"Serialization Test\""));
}

// ============================================================================
// Tag Tests
// ============================================================================

#[test]
fn test_tag_creation() {
    use systemprompt_content::models::Tag;
    use systemprompt_identifiers::TagId;

    let tag = Tag {
        id: TagId::new("tag-1"),
        name: "Rust".to_string(),
        slug: "rust".to_string(),
        created_at: None,
        updated_at: None,
    };

    assert_eq!(tag.name, "Rust");
    assert_eq!(tag.slug, "rust");
}

#[test]
fn test_tag_clone() {
    use systemprompt_content::models::Tag;
    use systemprompt_identifiers::TagId;
    use chrono::Utc;

    let tag = Tag {
        id: TagId::new("tag-2"),
        name: "Programming".to_string(),
        slug: "programming".to_string(),
        created_at: Some(Utc::now()),
        updated_at: Some(Utc::now()),
    };

    let cloned = tag.clone();
    assert_eq!(cloned.id, tag.id);
    assert_eq!(cloned.name, tag.name);
    assert_eq!(cloned.slug, tag.slug);
}

#[test]
fn test_tag_serialization() {
    use systemprompt_content::models::Tag;
    use systemprompt_identifiers::TagId;

    let tag = Tag {
        id: TagId::new("tag-3"),
        name: "Tutorial".to_string(),
        slug: "tutorial".to_string(),
        created_at: None,
        updated_at: None,
    };

    let json = serde_json::to_string(&tag).unwrap();
    assert!(json.contains("\"name\":\"Tutorial\""));
    assert!(json.contains("\"slug\":\"tutorial\""));
}
