use std::collections::HashMap;

use systemprompt_identifiers::{CategoryId, SourceId};
use systemprompt_models::content_config::{
    Category, ContentConfigError, ContentConfigErrors, ContentConfigRaw, ContentRouting,
    ContentSourceConfigRaw, IndexingConfig, Metadata, OrganizationData, ParentRoute, SitemapConfig,
    SourceBranding, StructuredData,
};

fn make_config() -> ContentConfigRaw {
    let sitemap = SitemapConfig {
        enabled: true,
        url_pattern: "/blog/{slug}".to_owned(),
        priority: 0.7,
        changefreq: "weekly".to_owned(),
        fetch_from: String::new(),
        parent_route: None,
    };
    let source = ContentSourceConfigRaw {
        path: "/srv/content/blog".to_owned(),
        source_id: SourceId::new("blog"),
        category_id: CategoryId::new("news"),
        enabled: true,
        description: String::new(),
        allowed_content_types: vec![],
        indexing: Some(IndexingConfig::default()),
        sitemap: Some(sitemap),
        branding: Some(SourceBranding::default()),
    };
    let mut content_sources = HashMap::new();
    content_sources.insert("blog".to_owned(), source);
    ContentConfigRaw {
        content_sources,
        metadata: Metadata::default(),
        categories: HashMap::new(),
    }
}

#[test]
fn matches_url_pattern_handles_slug_placeholder() {
    assert!(ContentConfigRaw::matches_url_pattern(
        "/blog/{slug}",
        "/blog/foo"
    ));
    assert!(ContentConfigRaw::matches_url_pattern(
        "/{slug}",
        "/anything"
    ));
    assert!(!ContentConfigRaw::matches_url_pattern(
        "/blog/{slug}",
        "/news/foo"
    ));
    assert!(!ContentConfigRaw::matches_url_pattern(
        "/blog/{slug}",
        "/blog/foo/bar"
    ));
}

#[test]
fn matches_url_pattern_ignores_trailing_slashes() {
    assert!(ContentConfigRaw::matches_url_pattern(
        "/blog/{slug}",
        "/blog/foo/"
    ));
}

#[test]
fn matches_url_pattern_handles_exact_segments() {
    assert!(ContentConfigRaw::matches_url_pattern("/about", "/about"));
    assert!(!ContentConfigRaw::matches_url_pattern("/about", "/contact"));
}

#[test]
fn is_html_page_returns_true_for_root_path() {
    let cfg = ContentConfigRaw::default();
    assert!(cfg.is_html_page("/"));
}

#[test]
fn is_html_page_matches_sitemap_pattern() {
    let cfg = make_config();
    assert!(cfg.is_html_page("/blog/post-1"));
}

#[test]
fn is_html_page_filters_well_known_paths() {
    let cfg = ContentConfigRaw::default();
    assert!(!cfg.is_html_page("/api/users"));
    assert!(!cfg.is_html_page("/track/x"));
    assert!(!cfg.is_html_page("/.well-known/jwks.json"));
    assert!(!cfg.is_html_page("/style.css"));
    assert!(cfg.is_html_page("/about"));
}

#[test]
fn determine_source_ignores_disabled_sources_for_routing() {
    let mut cfg = make_config();
    cfg.content_sources.get_mut("blog").unwrap().enabled = false;
    assert_eq!(cfg.determine_source("/blog/post-1"), "unknown");
}

#[test]
fn determine_source_returns_web_for_root() {
    let cfg = ContentConfigRaw::default();
    assert_eq!(cfg.determine_source("/"), "web");
}

#[test]
fn determine_source_returns_matching_name() {
    let cfg = make_config();
    assert_eq!(cfg.determine_source("/blog/post-1"), "blog");
}

#[test]
fn determine_source_returns_unknown_for_no_match() {
    let cfg = make_config();
    assert_eq!(cfg.determine_source("/news/x"), "unknown");
}

#[test]
fn resolve_slug_extracts_after_prefix() {
    let cfg = make_config();
    assert_eq!(
        cfg.resolve_slug("/blog/my-post"),
        Some("my-post".to_owned())
    );
    assert_eq!(
        cfg.resolve_slug("/blog/post-2?ref=a#section"),
        Some("post-2".to_owned())
    );
}

#[test]
fn resolve_slug_returns_none_when_no_pattern_matches() {
    let cfg = make_config();
    assert!(cfg.resolve_slug("/news/foo").is_none());
}

#[test]
fn resolve_slug_returns_none_when_slug_empty() {
    let cfg = make_config();
    assert!(cfg.resolve_slug("/blog/").is_none());
}

#[test]
fn resolve_slug_via_arc_trait_object() {
    use std::sync::Arc;
    let cfg: Arc<dyn ContentRouting> = Arc::new(make_config());
    assert_eq!(cfg.resolve_slug("/blog/x"), Some("x".to_owned()));
    assert!(cfg.is_html_page("/blog/x"));
    assert_eq!(cfg.determine_source("/blog/x"), "blog");
}

#[test]
fn content_config_errors_collects_and_emits() {
    let mut errs = ContentConfigErrors::new();
    assert!(errs.is_empty());
    errs.push(ContentConfigError::Io {
        path: std::path::PathBuf::from("/p"),
        message: "boom".to_owned(),
    });
    errs.push(ContentConfigError::Validation {
        field: "name".to_owned(),
        message: "missing".to_owned(),
        suggestion: None,
    });
    assert!(!errs.is_empty());
    assert_eq!(errs.errors().len(), 2);

    let display = format!("{errs}");
    assert!(display.contains("boom"));
    assert!(display.contains("missing"));
}

#[test]
fn content_config_errors_into_result_keeps_ok() {
    let errs = ContentConfigErrors::new();
    assert!(errs.into_result(42).is_ok());

    let mut errs = ContentConfigErrors::new();
    errs.push(ContentConfigError::Parse {
        path: std::path::PathBuf::from("/p"),
        message: "x".to_owned(),
    });
    assert!(errs.into_result(0).is_err());
}

#[test]
fn content_config_yaml_round_trip() {
    let yaml = r#"
content_sources:
  blog:
    path: /srv/blog
    source_id: blog
    category_id: news
    enabled: true
    sitemap:
      enabled: true
      url_pattern: /blog/{slug}
      priority: 0.5
      changefreq: weekly
metadata:
  default_author: Ed
  structured_data:
    organization:
      name: example
      url: https://example.org
      logo: /logo.png
categories:
  news:
    name: News
    description: News and updates
"#;
    let cfg: ContentConfigRaw = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(cfg.metadata.default_author, "Ed");
    assert_eq!(cfg.metadata.structured_data.organization.name, "example");
    assert_eq!(cfg.categories.get("news").unwrap().name, "News");
    assert_eq!(cfg.content_sources.get("blog").unwrap().path, "/srv/blog");
}

#[test]
fn parent_route_struct_constructs() {
    let pr = ParentRoute {
        enabled: true,
        url: "/blog".to_owned(),
        priority: 0.8,
        changefreq: "daily".to_owned(),
    };
    assert!(pr.enabled);
}

#[test]
fn structured_data_default_is_blank() {
    let sd = StructuredData::default();
    assert!(sd.organization.name.is_empty());
    assert!(sd.article.article_type.is_empty());

    let od = OrganizationData::default();
    assert!(od.url.is_empty());
}

#[test]
fn category_default_is_empty() {
    let c = Category::default();
    assert!(c.name.is_empty());
    assert!(c.description.is_empty());
}
