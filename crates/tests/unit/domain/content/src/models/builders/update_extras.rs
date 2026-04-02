use chrono::{TimeZone, Utc};
use systemprompt_content::models::{CategoryIdUpdate, UpdateContentParams};
use systemprompt_identifiers::{CategoryId, ContentId};

#[test]
fn with_public_some_true() {
    let params = UpdateContentParams::new(
        ContentId::new("id"),
        "Title".to_string(),
        "Desc".to_string(),
        "Body".to_string(),
    )
    .with_public(Some(true));

    assert_eq!(params.public, Some(true));
}

#[test]
fn with_public_some_false() {
    let params = UpdateContentParams::new(
        ContentId::new("id"),
        "Title".to_string(),
        "Desc".to_string(),
        "Body".to_string(),
    )
    .with_public(Some(false));

    assert_eq!(params.public, Some(false));
}

#[test]
fn with_public_none_leaves_unset() {
    let params = UpdateContentParams::new(
        ContentId::new("id"),
        "Title".to_string(),
        "Desc".to_string(),
        "Body".to_string(),
    )
    .with_public(None);

    assert!(params.public.is_none());
}

#[test]
fn with_kind_sets_value() {
    let params = UpdateContentParams::new(
        ContentId::new("id"),
        "Title".to_string(),
        "Desc".to_string(),
        "Body".to_string(),
    )
    .with_kind(Some("guide".to_string()));

    assert_eq!(params.kind, Some("guide".to_string()));
}

#[test]
fn with_kind_none_leaves_unset() {
    let params = UpdateContentParams::new(
        ContentId::new("id"),
        "Title".to_string(),
        "Desc".to_string(),
        "Body".to_string(),
    )
    .with_kind(None);

    assert!(params.kind.is_none());
}

#[test]
fn with_author_sets_value() {
    let params = UpdateContentParams::new(
        ContentId::new("id"),
        "Title".to_string(),
        "Desc".to_string(),
        "Body".to_string(),
    )
    .with_author(Some("Jane".to_string()));

    assert_eq!(params.author, Some("Jane".to_string()));
}

#[test]
fn with_published_at_sets_date() {
    let date = Utc.with_ymd_and_hms(2025, 6, 15, 0, 0, 0).unwrap();
    let params = UpdateContentParams::new(
        ContentId::new("id"),
        "Title".to_string(),
        "Desc".to_string(),
        "Body".to_string(),
    )
    .with_published_at(Some(date));

    assert_eq!(params.published_at, Some(date));
}

#[test]
fn with_links_sets_json() {
    let links = serde_json::json!([{"title": "Link", "url": "https://example.com"}]);
    let params = UpdateContentParams::new(
        ContentId::new("id"),
        "Title".to_string(),
        "Desc".to_string(),
        "Body".to_string(),
    )
    .with_links(Some(links.clone()));

    assert_eq!(params.links, Some(links));
}

#[test]
fn with_category_id_set() {
    let params = UpdateContentParams::new(
        ContentId::new("id"),
        "Title".to_string(),
        "Desc".to_string(),
        "Body".to_string(),
    )
    .with_category_id(Some(Some(CategoryId::new("tech"))));

    assert!(matches!(params.category_id, CategoryIdUpdate::Set(_)));
}

#[test]
fn with_category_id_clear() {
    let params = UpdateContentParams::new(
        ContentId::new("id"),
        "Title".to_string(),
        "Desc".to_string(),
        "Body".to_string(),
    )
    .with_category_id(Some(None::<CategoryId>));

    assert!(matches!(params.category_id, CategoryIdUpdate::Clear));
}

#[test]
fn full_builder_chain_all_optional_fields() {
    let date = Utc.with_ymd_and_hms(2025, 3, 1, 0, 0, 0).unwrap();
    let links = serde_json::json!([]);

    let params = UpdateContentParams::new(
        ContentId::new("full-id"),
        "Full Title".to_string(),
        "Full Desc".to_string(),
        "Full Body".to_string(),
    )
    .with_keywords("kw1, kw2".to_string())
    .with_image(Some("/img.png".to_string()))
    .with_version_hash("hash123".to_string())
    .with_category_id(Some(Some(CategoryId::new("cat"))))
    .with_public(Some(true))
    .with_kind(Some("tutorial".to_string()))
    .with_author(Some("Author".to_string()))
    .with_published_at(Some(date))
    .with_links(Some(links));

    assert_eq!(params.id.as_str(), "full-id");
    assert_eq!(params.keywords, "kw1, kw2");
    assert_eq!(params.image, Some("/img.png".to_string()));
    assert_eq!(params.version_hash, "hash123");
    assert_eq!(params.public, Some(true));
    assert_eq!(params.kind, Some("tutorial".to_string()));
    assert_eq!(params.author, Some("Author".to_string()));
    assert_eq!(params.published_at, Some(date));
    assert!(params.links.is_some());
}
