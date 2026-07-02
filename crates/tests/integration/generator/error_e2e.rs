//! Exercises the public `PublishError` constructor surface and its
//! accessor helpers (`location`, `suggestion_string`, `cause_string`),
//! including every branch of the suggestion-fix heuristic.

use std::path::PathBuf;
use systemprompt_generator::PublishError;

#[test]
fn missing_field_carries_suggestion_for_known_fields() {
    let cases = [
        ("image", "Add 'image:"),
        ("cover_image", "Add 'image:"),
        ("published_at", "Add 'date:"),
        ("date", "Add 'date:"),
        ("created_at", "Add 'date:"),
        ("author", "Add 'author:"),
        ("title", "Add 'title:"),
        ("slug", "Add 'slug:"),
        ("content_type", "kind"),
    ];
    for (field, fragment) in cases {
        let err = PublishError::missing_field(field, "my-slug");
        let suggestion = err
            .suggestion_string()
            .unwrap_or_else(|| panic!("expected suggestion for {field}"));
        assert!(
            suggestion.contains(fragment),
            "field {field} suggestion {suggestion:?} must contain {fragment}"
        );
    }
}

#[test]
fn missing_field_namespaced_fields_get_path_specific_suggestion() {
    let err = PublishError::missing_field("organization.name", "any");
    let suggestion = err.suggestion_string().expect("organization.* suggestion");
    assert!(suggestion.contains("name"));
    assert!(suggestion.contains("organization"));

    let err = PublishError::missing_field("article.published_time", "any");
    let suggestion = err.suggestion_string().expect("article.* suggestion");
    assert!(suggestion.contains("article"));

    let err = PublishError::missing_field("branding.logo", "any");
    let suggestion = err.suggestion_string().expect("branding.* suggestion");
    assert!(suggestion.contains("branding"));
    assert!(suggestion.contains("web.yaml"));
}

#[test]
fn missing_field_unknown_field_has_no_suggestion() {
    let err = PublishError::missing_field("totally_unknown", "x");
    assert!(err.suggestion_string().is_none());
}

#[test]
fn missing_field_with_path_exposes_location() {
    let path = PathBuf::from("/tmp/post.md");
    let err = PublishError::missing_field_with_path("title", "post", path.clone());
    assert_eq!(
        err.location().as_deref(),
        Some(path.display().to_string().as_str())
    );
}

#[test]
fn template_not_found_suggestion_lists_available() {
    let err = PublishError::template_not_found(
        "blog",
        "intro",
        vec!["post".to_owned(), "page".to_owned()],
    );
    let s = err.suggestion_string().expect("suggestion");
    assert!(s.contains("blog"));
    assert!(s.contains("post"));
    assert!(s.contains("page"));
}

#[test]
fn template_not_found_with_empty_available_recommends_adding() {
    let err = PublishError::template_not_found("blog", "intro", Vec::new());
    let s = err.suggestion_string().expect("suggestion");
    assert!(s.to_lowercase().contains("add"));
}

#[test]
fn provider_failed_cause_is_visible() {
    let err = PublishError::provider_failed("my-provider", "boom");
    assert_eq!(err.cause_string().as_deref(), Some("boom"));
    assert!(err.suggestion_string().is_none());
    assert!(err.to_string().contains("my-provider"));
}

#[test]
fn render_failed_cause_is_visible() {
    let err = PublishError::render_failed("tpl", Some("slug".to_owned()), "kaboom");
    assert_eq!(err.cause_string().as_deref(), Some("kaboom"));
    assert!(err.to_string().contains("tpl"));
    assert!(err.location().is_none());
}

#[test]
fn fetch_failed_cause_is_visible() {
    let err = PublishError::fetch_failed("src", "db-down");
    assert_eq!(err.cause_string().as_deref(), Some("db-down"));
}

#[test]
fn page_prerenderer_failed_cause_is_visible() {
    let err = PublishError::page_prerenderer_failed("home", "panicked");
    assert_eq!(err.cause_string().as_deref(), Some("panicked"));
}

#[test]
fn config_error_location_is_none_by_default() {
    let err = PublishError::config("bad value");
    assert!(err.location().is_none());
    assert!(err.suggestion_string().is_none());
    assert!(err.cause_string().is_none());
}

#[test]
fn io_yaml_json_have_no_extra_metadata() {
    let io: PublishError = std::io::Error::new(std::io::ErrorKind::NotFound, "x").into();
    assert!(io.location().is_none());
    assert!(io.suggestion_string().is_none());
    assert!(io.cause_string().is_none());

    let io_ctx = PublishError::io_context(
        "misc",
        std::io::Error::new(std::io::ErrorKind::NotFound, "x"),
    );
    assert_eq!(io_ctx.to_string(), "misc: x");
    assert!(io_ctx.cause_string().is_none());

    let yaml_err: Result<serde_yaml::Value, _> = serde_yaml::from_str("a:\n  - b\n - c");
    let yaml: PublishError = yaml_err.unwrap_err().into();
    assert!(yaml.to_string().to_lowercase().contains("yaml"));

    let json_err: Result<serde_json::Value, _> = serde_json::from_str("{ not json");
    let json: PublishError = json_err.unwrap_err().into();
    assert!(json.to_string().to_lowercase().contains("json"));
}
