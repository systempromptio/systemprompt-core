//! Tests for content.rs: ContentFilter, ContentSummary, ContentItem, and RepositoryError.

use chrono::Utc;
use systemprompt_identifiers::{ContentId, SourceId};
use systemprompt_traits::content::{ContentFilter, ContentItem, ContentSummary};
use systemprompt_traits::repository::RepositoryError;
use systemprompt_traits::context_provider::ContextProviderError;

// --- ContentFilter ---

#[test]
fn content_filter_default_is_all_none() {
    let f = ContentFilter::default();
    assert!(f.source_id.is_none());
    assert!(f.category_id.is_none());
    assert!(f.kind.is_none());
    assert!(f.query.is_none());
    assert!(f.limit.is_none());
    assert!(f.offset.is_none());
}

#[test]
fn content_filter_with_fields_set() {
    let src = SourceId::new("blog");
    let f = ContentFilter {
        source_id: Some(src.clone()),
        category_id: Some("tech".into()),
        kind: Some("guide".into()),
        query: Some("rust".into()),
        limit: Some(10),
        offset: Some(5),
    };
    assert_eq!(f.source_id.as_ref().unwrap(), &src);
    assert_eq!(f.category_id.as_deref(), Some("tech"));
    assert_eq!(f.kind.as_deref(), Some("guide"));
    assert_eq!(f.query.as_deref(), Some("rust"));
    assert_eq!(f.limit, Some(10));
    assert_eq!(f.offset, Some(5));
}

#[test]
fn content_filter_partial_eq() {
    let a = ContentFilter::default();
    let b = ContentFilter::default();
    assert_eq!(a, b);

    let c = ContentFilter {
        kind: Some("guide".into()),
        ..Default::default()
    };
    assert_ne!(a, c);
}

#[test]
fn content_filter_clone_is_equal() {
    let f = ContentFilter {
        kind: Some("blog".into()),
        limit: Some(20),
        ..Default::default()
    };
    let f2 = f.clone();
    assert_eq!(f, f2);
}

#[test]
fn content_filter_serde_roundtrip() {
    let f = ContentFilter {
        kind: Some("docs".into()),
        limit: Some(5),
        ..Default::default()
    };
    let json = serde_json::to_string(&f).unwrap();
    let f2: ContentFilter = serde_json::from_str(&json).unwrap();
    assert_eq!(f, f2);
}

// --- ContentSummary ---

#[test]
fn content_summary_fields_accessible() {
    let s = ContentSummary {
        id: ContentId::new("c1"),
        slug: "my-post".into(),
        title: "My Post".into(),
        description: "A great post".into(),
        published_at: Utc::now(),
        kind: "guide".into(),
        source_id: SourceId::new("blog"),
    };
    assert_eq!(s.slug, "my-post");
    assert_eq!(s.title, "My Post");
    assert_eq!(s.kind, "guide");
}

#[test]
fn content_summary_clone_preserves_id() {
    let id = ContentId::new("abc");
    let s = ContentSummary {
        id: id.clone(),
        slug: "slug".into(),
        title: "Title".into(),
        description: "Desc".into(),
        published_at: Utc::now(),
        kind: "page".into(),
        source_id: SourceId::new("src"),
    };
    let s2 = s.clone();
    assert_eq!(s2.id, id);
}

#[test]
fn content_summary_serde_roundtrip() {
    let s = ContentSummary {
        id: ContentId::new("id1"),
        slug: "test".into(),
        title: "Test".into(),
        description: "Desc".into(),
        published_at: Utc::now(),
        kind: "guide".into(),
        source_id: SourceId::new("src"),
    };
    let json = serde_json::to_string(&s).unwrap();
    let s2: ContentSummary = serde_json::from_str(&json).unwrap();
    assert_eq!(s2.slug, "test");
    assert_eq!(s2.title, "Test");
}

// --- ContentItem ---

#[test]
fn content_item_fields_accessible() {
    let item = ContentItem {
        id: ContentId::new("i1"),
        slug: "intro".into(),
        title: "Introduction".into(),
        description: "An intro".into(),
        body: "Body text here".into(),
        author: "Alice".into(),
        published_at: Utc::now(),
        keywords: "rust, intro".into(),
        kind: "guide".into(),
        image: Some("cover.png".into()),
        source_id: SourceId::new("docs"),
        category_id: Some("tutorials".into()),
    };
    assert_eq!(item.author, "Alice");
    assert_eq!(item.body, "Body text here");
    assert_eq!(item.image.as_deref(), Some("cover.png"));
    assert_eq!(item.category_id.as_deref(), Some("tutorials"));
}

#[test]
fn content_item_optional_fields_can_be_none() {
    let item = ContentItem {
        id: ContentId::new("i2"),
        slug: "bare".into(),
        title: "Bare".into(),
        description: "No image or category".into(),
        body: "".into(),
        author: "Bob".into(),
        published_at: Utc::now(),
        keywords: "".into(),
        kind: "page".into(),
        image: None,
        source_id: SourceId::new("site"),
        category_id: None,
    };
    assert!(item.image.is_none());
    assert!(item.category_id.is_none());
}

#[test]
fn content_item_serde_roundtrip() {
    let item = ContentItem {
        id: ContentId::new("id2"),
        slug: "serde-test".into(),
        title: "Serde Test".into(),
        description: "Testing serde".into(),
        body: "some body".into(),
        author: "Author".into(),
        published_at: Utc::now(),
        keywords: "key".into(),
        kind: "blog".into(),
        image: None,
        source_id: SourceId::new("s"),
        category_id: None,
    };
    let json = serde_json::to_string(&item).unwrap();
    let item2: ContentItem = serde_json::from_str(&json).unwrap();
    assert_eq!(item2.slug, "serde-test");
    assert_eq!(item2.author, "Author");
}

// --- RepositoryError ---

#[test]
fn repository_not_found_display() {
    let e = RepositoryError::NotFound("user-1".to_owned());
    assert!(format!("{e}").contains("user-1"));
}

#[test]
fn repository_invalid_data_display() {
    let e = RepositoryError::InvalidData("bad uuid".to_owned());
    assert!(format!("{e}").contains("bad uuid"));
}

#[test]
fn repository_constraint_violation_display() {
    let e = RepositoryError::ConstraintViolation("unique_email".to_owned());
    assert!(format!("{e}").contains("unique_email"));
}

#[test]
fn repository_internal_display() {
    let e = RepositoryError::Internal("unexpected panic".to_owned());
    assert!(format!("{e}").contains("unexpected panic"));
}

#[test]
fn repository_database_constructor() {
    let io_err = std::io::Error::new(std::io::ErrorKind::Other, "db down");
    let e = RepositoryError::database(io_err);
    assert!(format!("{e}").contains("database error"));
    assert!(format!("{e}").contains("db down"));
}

#[test]
fn repository_serialization_from_json_error() {
    let json_err: serde_json::Error = serde_json::from_str::<i32>("not json").unwrap_err();
    let e: RepositoryError = json_err.into();
    let s = format!("{e}");
    assert!(s.contains("serialization"));
}

#[test]
fn repository_error_is_std_error() {
    let e: Box<dyn std::error::Error> =
        Box::new(RepositoryError::Internal("test".into()));
    assert!(!e.to_string().is_empty());
}

// --- ContextProviderError display ---

#[test]
fn context_provider_not_found_display() {
    let e = ContextProviderError::NotFound("ctx-42".to_owned());
    assert!(format!("{e}").contains("ctx-42"));
}

#[test]
fn context_provider_access_denied_display() {
    let e = ContextProviderError::AccessDenied("unauthorized".to_owned());
    assert!(format!("{e}").contains("unauthorized"));
}

#[test]
fn context_provider_database_display() {
    let e = ContextProviderError::Database("connection lost".to_owned());
    assert!(format!("{e}").contains("connection lost"));
}

#[test]
fn context_provider_internal_display() {
    let e = ContextProviderError::Internal("panic in handler".to_owned());
    assert!(format!("{e}").contains("panic in handler"));
}
