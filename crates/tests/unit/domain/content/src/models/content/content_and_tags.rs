//! Tests for Content, ContentSummary, and Tag types.

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
