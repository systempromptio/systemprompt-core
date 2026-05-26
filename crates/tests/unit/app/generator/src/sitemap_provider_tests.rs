//! Unit tests for DefaultSitemapProvider via `from_config`.
//!
//! These exercise `source_specs`, `static_urls`, `resolve_placeholders`,
//! `provider_id`, and the source-filtering logic (enabled flag, sitemap
//! enabled flag, parent_route handling).

use std::collections::HashMap;
use systemprompt_generator::DefaultSitemapProvider;
use systemprompt_identifiers::{CategoryId, SourceId};
use systemprompt_models::{
    ContentConfigRaw, ContentSourceConfigRaw, ParentRoute, SitemapConfig, SourceBranding,
};
use systemprompt_provider_contracts::{
    PlaceholderMapping, SitemapContext, SitemapProvider,
};

fn source(
    source_id: &str,
    enabled: bool,
    sitemap: Option<SitemapConfig>,
) -> ContentSourceConfigRaw {
    ContentSourceConfigRaw {
        path: "/content".to_string(),
        source_id: SourceId::new(source_id),
        category_id: CategoryId::new("default"),
        enabled,
        description: String::new(),
        allowed_content_types: Vec::new(),
        indexing: None,
        sitemap,
        branding: Some(SourceBranding::default()),
    }
}

fn basic_sitemap(url_pattern: &str, parent: Option<ParentRoute>) -> SitemapConfig {
    SitemapConfig {
        enabled: true,
        url_pattern: url_pattern.to_string(),
        priority: 0.8,
        changefreq: "weekly".to_string(),
        fetch_from: String::new(),
        parent_route: parent,
    }
}

fn config_with(sources: Vec<(&str, ContentSourceConfigRaw)>) -> ContentConfigRaw {
    let mut map = HashMap::new();
    for (name, src) in sources {
        map.insert(name.to_string(), src);
    }
    ContentConfigRaw {
        content_sources: map,
        ..Default::default()
    }
}

#[test]
fn provider_id_is_stable() {
    let cfg = ContentConfigRaw::default();
    let p = DefaultSitemapProvider::from_config(cfg);
    assert_eq!(p.provider_id(), "default-sitemap");
}

#[test]
fn source_specs_empty_when_no_sources() {
    let cfg = ContentConfigRaw::default();
    let p = DefaultSitemapProvider::from_config(cfg);
    assert!(p.source_specs().is_empty());
}

#[test]
fn source_specs_emits_enabled_sources_with_sitemap() {
    let cfg = config_with(vec![(
        "blog",
        source("blog", true, Some(basic_sitemap("/blog/{slug}", None))),
    )]);
    let p = DefaultSitemapProvider::from_config(cfg);
    let specs = p.source_specs();
    assert_eq!(specs.len(), 1);
    assert_eq!(specs[0].source_id.as_str(), "blog");
    assert_eq!(specs[0].url_pattern, "/blog/{slug}");
    assert_eq!(specs[0].priority, 0.8);
    assert_eq!(specs[0].changefreq, "weekly");
    assert_eq!(specs[0].placeholders.len(), 1);
    assert_eq!(specs[0].placeholders[0].placeholder, "{slug}");
    assert_eq!(specs[0].placeholders[0].field, "slug");
}

#[test]
fn source_specs_skips_disabled_source() {
    let cfg = config_with(vec![(
        "blog",
        source("blog", false, Some(basic_sitemap("/blog/{slug}", None))),
    )]);
    let p = DefaultSitemapProvider::from_config(cfg);
    assert!(p.source_specs().is_empty());
}

#[test]
fn source_specs_skips_source_without_sitemap() {
    let cfg = config_with(vec![("x", source("x", true, None))]);
    let p = DefaultSitemapProvider::from_config(cfg);
    assert!(p.source_specs().is_empty());
}

#[test]
fn source_specs_skips_sitemap_with_enabled_false() {
    let mut sm = basic_sitemap("/x/{slug}", None);
    sm.enabled = false;
    let cfg = config_with(vec![("x", source("x", true, Some(sm)))]);
    let p = DefaultSitemapProvider::from_config(cfg);
    assert!(p.source_specs().is_empty());
}

#[test]
fn static_urls_empty_when_no_parent_route() {
    let cfg = config_with(vec![(
        "blog",
        source("blog", true, Some(basic_sitemap("/blog/{slug}", None))),
    )]);
    let p = DefaultSitemapProvider::from_config(cfg);
    assert!(p.static_urls("https://example.com").is_empty());
}

#[test]
fn static_urls_emits_enabled_parent_route() {
    let parent = ParentRoute {
        enabled: true,
        url: "/blog".to_string(),
        priority: 0.5,
        changefreq: "daily".to_string(),
    };
    let cfg = config_with(vec![(
        "blog",
        source(
            "blog",
            true,
            Some(basic_sitemap("/blog/{slug}", Some(parent))),
        ),
    )]);
    let p = DefaultSitemapProvider::from_config(cfg);
    let urls = p.static_urls("https://example.com");
    assert_eq!(urls.len(), 1);
    assert_eq!(urls[0].loc, "https://example.com/blog");
    assert_eq!(urls[0].priority, 0.5);
    assert_eq!(urls[0].changefreq, "daily");
    assert!(!urls[0].lastmod.is_empty());
}

#[test]
fn static_urls_skips_disabled_parent_route() {
    let parent = ParentRoute {
        enabled: false,
        url: "/x".to_string(),
        priority: 0.5,
        changefreq: "daily".to_string(),
    };
    let cfg = config_with(vec![(
        "x",
        source("x", true, Some(basic_sitemap("/x/{slug}", Some(parent)))),
    )]);
    let p = DefaultSitemapProvider::from_config(cfg);
    assert!(p.static_urls("https://example.com").is_empty());
}

#[test]
fn static_urls_skips_disabled_source() {
    let parent = ParentRoute {
        enabled: true,
        url: "/x".to_string(),
        priority: 0.5,
        changefreq: "daily".to_string(),
    };
    let cfg = config_with(vec![(
        "x",
        source("x", false, Some(basic_sitemap("/x/{slug}", Some(parent)))),
    )]);
    let p = DefaultSitemapProvider::from_config(cfg);
    assert!(p.static_urls("https://example.com").is_empty());
}

#[tokio::test]
async fn resolve_placeholders_string_field() {
    let p = DefaultSitemapProvider::from_config(ContentConfigRaw::default());
    let content = serde_json::json!({"slug": "hello-world"});
    let placeholders = vec![PlaceholderMapping {
        placeholder: "{slug}".to_string(),
        field: "slug".to_string(),
    }];
    let ctx = SitemapContext {
        base_url: "https://e.com",
        source_name: "src",
    };
    let resolved = p
        .resolve_placeholders(&ctx, &content, &placeholders)
        .await
        .unwrap();
    assert_eq!(resolved.get("{slug}").map(String::as_str), Some("hello-world"));
}

#[tokio::test]
async fn resolve_placeholders_numeric_field() {
    let p = DefaultSitemapProvider::from_config(ContentConfigRaw::default());
    let content = serde_json::json!({"id": 42});
    let placeholders = vec![PlaceholderMapping {
        placeholder: "{id}".to_string(),
        field: "id".to_string(),
    }];
    let ctx = SitemapContext {
        base_url: "https://e.com",
        source_name: "src",
    };
    let resolved = p
        .resolve_placeholders(&ctx, &content, &placeholders)
        .await
        .unwrap();
    assert_eq!(resolved.get("{id}").map(String::as_str), Some("42"));
}

#[tokio::test]
async fn resolve_placeholders_missing_field_is_skipped() {
    let p = DefaultSitemapProvider::from_config(ContentConfigRaw::default());
    let content = serde_json::json!({"other": "value"});
    let placeholders = vec![PlaceholderMapping {
        placeholder: "{slug}".to_string(),
        field: "slug".to_string(),
    }];
    let ctx = SitemapContext {
        base_url: "https://e.com",
        source_name: "src",
    };
    let resolved = p
        .resolve_placeholders(&ctx, &content, &placeholders)
        .await
        .unwrap();
    assert!(resolved.is_empty());
}

#[test]
fn debug_includes_struct_name() {
    let p = DefaultSitemapProvider::from_config(ContentConfigRaw::default());
    assert!(format!("{:?}", p).contains("DefaultSitemapProvider"));
}
