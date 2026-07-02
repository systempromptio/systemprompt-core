//! Tests for `web::content_types` — the config builders behind `create` and
//! the flag/`--set` application logic behind `edit`.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_identifiers::{CategoryId, SourceId};
use systemprompt_cli::web::content_types::builder::{
    SourceSpec, build_flag_sitemap, build_source_config, ensure_category_exists,
};
use systemprompt_cli::web::content_types::edit::{
    EditArgs, apply_basic_flags, apply_set_value_changes, apply_sitemap_flags,
};
use systemprompt_models::content_config::{ContentConfigRaw, ContentSourceConfigRaw};

fn config_with_categories(names: &[&str]) -> ContentConfigRaw {
    let cats = names
        .iter()
        .map(|n| format!("  {n}:\n    name: {n}\n"))
        .collect::<String>();
    let yaml = if names.is_empty() {
        "content_sources: {}\n".to_owned()
    } else {
        format!("content_sources: {{}}\ncategories:\n{cats}")
    };
    serde_yaml::from_str(&yaml).unwrap()
}

fn source(sitemap: bool) -> ContentSourceConfigRaw {
    let sitemap_yaml = if sitemap {
        "\nsitemap:\n  enabled: true\n  url_pattern: /blog/{slug}\n  priority: 0.5\n  changefreq: weekly\n"
    } else {
        "\n"
    };
    serde_yaml::from_str(&format!(
        "path: content/blog\nsource_id: blog\ncategory_id: blog\nenabled: true{sitemap_yaml}"
    ))
    .unwrap()
}

fn edit_args() -> EditArgs {
    EditArgs {
        name: None,
        set_values: vec![],
        enable: false,
        disable: false,
        url_pattern: None,
        priority: None,
        changefreq: None,
        path: None,
        description: None,
    }
}

#[test]
fn ensure_category_exists_accepts_known_category() {
    let config = config_with_categories(&["blog", "docs"]);
    assert!(ensure_category_exists(&config, "blog").is_ok());
}

#[test]
fn ensure_category_exists_rejects_unknown_and_lists_available() {
    let config = config_with_categories(&["blog"]);
    let err = ensure_category_exists(&config, "news")
        .unwrap_err()
        .to_string();
    assert!(err.contains("news"));
    assert!(err.contains("blog"));
}

#[test]
fn build_flag_sitemap_populates_database_entry() {
    let sitemap = build_flag_sitemap("/blog/{slug}".to_owned(), 0.8, "daily");
    assert!(sitemap.enabled);
    assert_eq!(sitemap.url_pattern, "/blog/{slug}");
    assert_eq!(sitemap.priority, 0.8);
    assert_eq!(sitemap.changefreq, "daily");
    assert_eq!(sitemap.fetch_from, "database");
    assert!(sitemap.parent_route.is_none());
}

#[test]
fn build_source_config_applies_article_defaults() {
    let source = build_source_config(SourceSpec {
        path: "content/news".to_owned(),
        source_id: SourceId::new("news"),
        category_id: CategoryId::new("blog"),
        enabled: true,
        description: "the news".to_owned(),
        sitemap: None,
    });
    assert_eq!(source.path, "content/news");
    assert_eq!(source.source_id.as_str(), "news");
    assert_eq!(source.category_id.as_str(), "blog");
    assert!(source.enabled);
    assert_eq!(source.allowed_content_types, vec!["article"]);
    let indexing = source.indexing.unwrap();
    assert!(indexing.recursive);
    assert!(!indexing.clear_before);
    assert!(!indexing.override_existing);
    assert!(source.sitemap.is_none());
    assert!(source.branding.is_none());
}

#[test]
fn apply_basic_flags_enable_disable_and_fields() {
    let mut src = source(false);
    let mut args = edit_args();
    args.disable = true;
    args.path = Some("content/other".to_owned());
    args.description = Some("desc".to_owned());
    let mut changes = Vec::new();
    apply_basic_flags(&mut src, &args, &mut changes);
    assert!(!src.enabled);
    assert_eq!(src.path, "content/other");
    assert_eq!(src.description, "desc");
    assert_eq!(changes.len(), 3);
}

#[test]
fn apply_basic_flags_noop_without_flags() {
    let mut src = source(false);
    let mut changes = Vec::new();
    apply_basic_flags(&mut src, &edit_args(), &mut changes);
    assert!(changes.is_empty());
}

#[test]
fn apply_sitemap_flags_requires_existing_sitemap() {
    let mut src = source(false);
    let mut args = edit_args();
    args.priority = Some(0.9);
    let mut changes = Vec::new();
    let err = apply_sitemap_flags(&mut src, &args, &mut changes, "blog").unwrap_err();
    assert!(err.to_string().contains("no sitemap configuration"));
}

#[test]
fn apply_sitemap_flags_is_noop_without_sitemap_args() {
    let mut src = source(false);
    let mut changes = Vec::new();
    assert!(apply_sitemap_flags(&mut src, &edit_args(), &mut changes, "blog").is_ok());
    assert!(changes.is_empty());
}

#[test]
fn apply_sitemap_flags_updates_all_fields() {
    let mut src = source(true);
    let mut args = edit_args();
    args.url_pattern = Some("/news/{slug}".to_owned());
    args.priority = Some(0.7);
    args.changefreq = Some("daily".to_owned());
    let mut changes = Vec::new();
    apply_sitemap_flags(&mut src, &args, &mut changes, "blog").unwrap();
    let sitemap = src.sitemap.as_ref().unwrap();
    assert_eq!(sitemap.url_pattern, "/news/{slug}");
    assert_eq!(sitemap.priority, 0.7);
    assert_eq!(sitemap.changefreq, "daily");
    assert_eq!(changes.len(), 3);
}

#[test]
fn apply_sitemap_flags_rejects_out_of_range_priority() {
    let mut src = source(true);
    let mut args = edit_args();
    args.priority = Some(1.5);
    let mut changes = Vec::new();
    let err = apply_sitemap_flags(&mut src, &args, &mut changes, "blog").unwrap_err();
    assert!(err.to_string().contains("between 0.0 and 1.0"));
}

#[test]
fn set_values_update_scalar_keys() {
    let mut src = source(true);
    let mut changes = Vec::new();
    apply_set_value_changes(
        &mut src,
        &[
            "description=new desc".to_owned(),
            "path=content/x".to_owned(),
            "enabled=false".to_owned(),
            "sitemap.url_pattern=/x/{slug}".to_owned(),
            "sitemap.priority=0.3".to_owned(),
            "sitemap.changefreq=monthly".to_owned(),
        ],
        &mut changes,
    )
    .unwrap();
    assert_eq!(src.description, "new desc");
    assert_eq!(src.path, "content/x");
    assert!(!src.enabled);
    let sitemap = src.sitemap.as_ref().unwrap();
    assert_eq!(sitemap.url_pattern, "/x/{slug}");
    assert_eq!(sitemap.priority, 0.3);
    assert_eq!(sitemap.changefreq, "monthly");
    assert_eq!(changes.len(), 6);
}

#[test]
fn set_values_preserve_equals_in_value() {
    let mut src = source(false);
    let mut changes = Vec::new();
    apply_set_value_changes(&mut src, &["description=a=b".to_owned()], &mut changes).unwrap();
    assert_eq!(src.description, "a=b");
}

#[test]
fn set_values_reject_malformed_pair() {
    let mut src = source(false);
    let mut changes = Vec::new();
    let err = apply_set_value_changes(&mut src, &["no-equals".to_owned()], &mut changes)
        .unwrap_err()
        .to_string();
    assert!(err.contains("Expected key=value"));
}

#[test]
fn set_values_reject_unknown_key() {
    let mut src = source(false);
    let mut changes = Vec::new();
    let err = apply_set_value_changes(&mut src, &["bogus=1".to_owned()], &mut changes)
        .unwrap_err()
        .to_string();
    assert!(err.contains("Unknown configuration key"));
}

#[test]
fn set_values_reject_bad_boolean_and_float() {
    let mut src = source(true);
    let mut changes = Vec::new();
    assert!(
        apply_set_value_changes(&mut src, &["enabled=maybe".to_owned()], &mut changes).is_err()
    );
    assert!(
        apply_set_value_changes(
            &mut src,
            &["sitemap.priority=high".to_owned()],
            &mut changes
        )
        .is_err()
    );
}

#[test]
fn set_values_on_sitemap_keys_fail_without_sitemap() {
    let mut src = source(false);
    let mut changes = Vec::new();
    let err = apply_set_value_changes(
        &mut src,
        &["sitemap.changefreq=daily".to_owned()],
        &mut changes,
    )
    .unwrap_err()
    .to_string();
    assert!(err.contains("No sitemap configuration"));
}
