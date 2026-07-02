//! Unit tests for generator error types and suggestion heuristics.
//!
//! `error/suggestions.rs` is private — covered transitively via the
//! `missing_field` / `missing_field_with_path` constructors which call
//! `suggest_fix_for_field` under the hood.

use std::path::PathBuf;
use systemprompt_generator::PublishError;

#[test]
fn missing_field_sets_suggestion_for_image() {
    let err = PublishError::missing_field("image", "my-post");
    let s = err.to_string();
    assert!(s.contains("Missing field 'image'"));
    assert!(s.contains("my-post"));
    let sugg = err.suggestion_string().expect("image has suggestion");
    assert!(sugg.contains("placeholder.svg"));
}

#[test]
fn missing_field_cover_image_suggestion() {
    let err = PublishError::missing_field("cover_image", "post");
    assert!(err.suggestion_string().is_some());
}

#[test]
fn missing_field_published_at_suggestion() {
    for field in [
        "published_at",
        "date",
        "created_at",
        "published_at/date/created_at",
    ] {
        let err = PublishError::missing_field(field, "slug");
        let sugg = err
            .suggestion_string()
            .expect("date variant has suggestion");
        assert!(sugg.contains("YYYY-MM-DD"));
    }
}

#[test]
fn missing_field_author_title_slug() {
    for (field, needle) in [
        ("author", "Your Name"),
        ("title", "Your Title"),
        ("slug", "your-slug"),
        ("content_type", "kind"),
    ] {
        let err = PublishError::missing_field(field, "x");
        let sugg = err.suggestion_string().expect("known field has suggestion");
        assert!(sugg.contains(needle), "field={field}, sugg={sugg}");
    }
}

#[test]
fn missing_field_organization_prefix() {
    let err = PublishError::missing_field("organization.logo", "x");
    let sugg = err.suggestion_string().unwrap();
    assert!(sugg.contains("organization"));
    assert!(sugg.contains("logo"));
}

#[test]
fn missing_field_article_prefix() {
    let err = PublishError::missing_field("article.byline", "x");
    let sugg = err.suggestion_string().unwrap();
    assert!(sugg.contains("article"));
    assert!(sugg.contains("byline"));
}

#[test]
fn missing_field_branding_prefix() {
    let err = PublishError::missing_field("branding.tagline", "x");
    let sugg = err.suggestion_string().unwrap();
    assert!(sugg.contains("branding"));
    assert!(sugg.contains("tagline"));
    assert!(sugg.contains("web.yaml"));
}

#[test]
fn missing_field_unknown_returns_no_suggestion() {
    let err = PublishError::missing_field("nonsense_field_xyz", "x");
    assert!(err.suggestion_string().is_none());
}

#[test]
fn missing_field_with_path_records_location() {
    let path = PathBuf::from("/some/file.md");
    let err = PublishError::missing_field_with_path("title", "post", path.clone());
    assert_eq!(err.location(), Some(path.display().to_string()));
}

#[test]
fn missing_field_default_has_no_location() {
    let err = PublishError::missing_field("title", "post");
    assert!(err.location().is_none());
}

#[test]
fn template_not_found_with_alternatives() {
    let err = PublishError::template_not_found(
        "weird",
        "slug",
        vec!["article".to_string(), "post".to_string()],
    );
    let s = err.to_string();
    assert!(s.contains("'weird'"));
    let sugg = err.suggestion_string().unwrap();
    assert!(sugg.contains("article"));
    assert!(sugg.contains("post"));
}

#[test]
fn template_not_found_no_alternatives() {
    let err = PublishError::template_not_found("any", "slug", Vec::new());
    let sugg = err.suggestion_string().unwrap();
    assert!(sugg.contains("Add templates"));
}

#[test]
fn provider_failed_helper() {
    let err = PublishError::provider_failed("rss", "boom");
    let s = err.to_string();
    assert!(s.contains("rss"));
    assert!(s.contains("boom"));
    assert_eq!(err.cause_string(), Some("boom".to_string()));
    assert!(err.suggestion_string().is_none());
}

#[test]
fn render_failed_helper_with_slug() {
    let err = PublishError::render_failed("base.hbs", Some("hello".to_string()), "missing var");
    let s = err.to_string();
    assert!(s.contains("base.hbs"));
    assert_eq!(err.cause_string(), Some("missing var".to_string()));
}

#[test]
fn render_failed_helper_no_slug() {
    let err = PublishError::render_failed("base.hbs", None, "x");
    assert_eq!(err.cause_string(), Some("x".to_string()));
}

#[test]
fn fetch_failed_helper() {
    let err = PublishError::fetch_failed("blog", "http 500");
    assert!(err.to_string().contains("blog"));
    assert_eq!(err.cause_string(), Some("http 500".to_string()));
}

#[test]
fn config_helper() {
    let err = PublishError::config("bad value");
    assert!(err.to_string().contains("bad value"));
    assert!(err.location().is_none());
}

#[test]
fn page_prerenderer_failed_helper() {
    let err = PublishError::page_prerenderer_failed("homepage", "panic");
    assert!(err.to_string().contains("homepage"));
    assert_eq!(err.cause_string(), Some("panic".to_string()));
}

#[test]
fn io_context_helper() {
    let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "gone");
    let err = PublishError::io_context("Failed to copy style.css", io_err);
    assert_eq!(err.to_string(), "Failed to copy style.css: gone");
}

#[test]
fn io_from_conversion() {
    let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "no");
    let err: PublishError = io_err.into();
    assert!(err.to_string().starts_with("I/O error"));
    assert!(err.cause_string().is_none());
}

#[test]
fn yaml_from_conversion() {
    let yaml_err: serde_yaml::Error = serde_yaml::from_str::<i32>("not-int").unwrap_err();
    let err: PublishError = yaml_err.into();
    assert!(err.to_string().starts_with("YAML error"));
}

#[test]
fn json_from_conversion() {
    let json_err = serde_json::from_str::<serde_json::Value>("xxx").unwrap_err();
    let err: PublishError = json_err.into();
    assert!(err.to_string().starts_with("JSON error"));
}

#[test]
fn cause_string_none_for_missing_field() {
    let err = PublishError::missing_field("title", "x");
    assert!(err.cause_string().is_none());
}

#[test]
fn debug_includes_variant() {
    let err = PublishError::missing_field("title", "slug-1");
    let dbg = format!("{:?}", err);
    assert!(dbg.contains("MissingField"));
    assert!(dbg.contains("slug-1"));
}
