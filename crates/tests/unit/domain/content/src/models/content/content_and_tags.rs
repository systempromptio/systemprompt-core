//! Tests for Content, ContentSummary, and Tag types.

fn content_with_links(
    links: Vec<systemprompt_content::models::ContentLinkMetadata>,
) -> systemprompt_content::models::Content {
    use chrono::Utc;
    use systemprompt_content::models::Content;
    use systemprompt_identifiers::{ContentId, LocaleCode, SourceId};

    Content {
        id: ContentId::new("content-1"),
        slug: "test-content".to_string(),
        locale: LocaleCode::english(),
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
        links: sqlx::types::Json(links),
        updated_at: Utc::now(),
    }
}

#[test]
fn test_content_links_serialize_as_plain_array() {
    use systemprompt_content::models::ContentLinkMetadata;

    let content = content_with_links(vec![
        ContentLinkMetadata {
            title: "Link 1".to_string(),
            url: "https://example.com/1".to_string(),
        },
        ContentLinkMetadata {
            title: "Link 2".to_string(),
            url: "https://example.com/2".to_string(),
        },
    ]);

    let value = serde_json::to_value(&content).expect("serialize content");
    assert_eq!(
        value["links"],
        serde_json::json!([
            {"title": "Link 1", "url": "https://example.com/1"},
            {"title": "Link 2", "url": "https://example.com/2"}
        ])
    );
}

#[test]
fn test_content_links_default_to_empty_when_absent() {
    use systemprompt_content::models::Content;

    let mut value = serde_json::to_value(content_with_links(Vec::new())).expect("serialize");
    value.as_object_mut().expect("object").remove("links");

    let content: Content = serde_json::from_value(value).expect("links is optional");
    assert!(content.links.is_empty());
}

#[test]
fn test_content_links_reject_non_array() {
    use systemprompt_content::models::Content;

    let mut value = serde_json::to_value(content_with_links(Vec::new())).expect("serialize");
    value["links"] = serde_json::json!({"not": "an array"});

    serde_json::from_value::<Content>(value).unwrap_err();
}

#[test]
fn test_content_summary_creation() {
    use chrono::Utc;
    use systemprompt_content::models::ContentSummary;
    use systemprompt_identifiers::ContentId;

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
fn test_content_summary_serialization() {
    use chrono::Utc;
    use systemprompt_content::models::ContentSummary;
    use systemprompt_identifiers::ContentId;

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
