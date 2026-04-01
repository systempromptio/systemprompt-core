//! Tests for ContentMetadata serialization and ContentLinkMetadata.

use systemprompt_content::ContentMetadata;

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
        public: Some(true),
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
