//! Coverage for the `SitemapProvider` trait defaults and its value types.

use std::collections::HashMap;

use async_trait::async_trait;
use serde_json::{Value, json};
use systemprompt_identifiers::{LocaleCode, SourceId};
use systemprompt_provider_contracts::{
    PlaceholderMapping, ProviderResult, SitemapAlternate, SitemapContext, SitemapProvider,
    SitemapSourceSpec, SitemapUrlEntry,
};

struct MinimalSitemap;

#[async_trait]
impl SitemapProvider for MinimalSitemap {
    fn provider_id(&self) -> &'static str {
        "minimal"
    }

    async fn resolve_placeholders(
        &self,
        ctx: &SitemapContext<'_>,
        content: &Value,
        placeholders: &[PlaceholderMapping],
    ) -> ProviderResult<HashMap<String, String>> {
        let mut out = HashMap::new();
        out.insert("base".to_string(), ctx.base_url.to_string());
        for p in placeholders {
            if let Some(v) = content.get(&p.field).and_then(Value::as_str) {
                out.insert(p.placeholder.clone(), v.to_string());
            }
        }
        Ok(out)
    }
}

#[test]
fn trait_defaults_are_empty_and_priority_100() {
    let p = MinimalSitemap;
    assert_eq!(p.provider_id(), "minimal");
    assert!(p.source_specs().is_empty());
    assert!(p.static_urls("https://example.com").is_empty());
    assert_eq!(p.priority(), 100);
}

#[tokio::test]
async fn resolve_placeholders_maps_fields() {
    let ctx = SitemapContext {
        base_url: "https://example.com",
        source_name: "blog",
    };
    let content = json!({"slug": "hello-world"});
    let placeholders = vec![PlaceholderMapping {
        placeholder: "{slug}".to_string(),
        field: "slug".to_string(),
    }];

    let out = MinimalSitemap
        .resolve_placeholders(&ctx, &content, &placeholders)
        .await
        .unwrap();

    assert_eq!(
        out.get("base").map(String::as_str),
        Some("https://example.com")
    );
    assert_eq!(out.get("{slug}").map(String::as_str), Some("hello-world"));
}

#[test]
fn url_entry_and_alternate_construct() {
    let entry = SitemapUrlEntry {
        loc: "https://example.com/x".to_string(),
        lastmod: "2026-06-22".to_string(),
        changefreq: "daily".to_string(),
        priority: 0.8,
        alternates: vec![SitemapAlternate {
            hreflang: LocaleCode::new("de"),
            href: "https://example.com/de/x".to_string(),
        }],
    };

    assert_eq!(entry.priority, 0.8);
    assert_eq!(entry.alternates.len(), 1);
    assert_eq!(entry.alternates[0].hreflang.as_str(), "de");

    let cloned = entry.clone();
    assert_eq!(cloned.loc, entry.loc);
}

#[test]
fn source_spec_holds_typed_source_id() {
    let spec = SitemapSourceSpec {
        source_id: SourceId::new("blog"),
        url_pattern: "/blog/{slug}".to_string(),
        placeholders: vec![PlaceholderMapping {
            placeholder: "{slug}".to_string(),
            field: "slug".to_string(),
        }],
        priority: 0.5,
        changefreq: "weekly".to_string(),
    };

    assert_eq!(spec.source_id.as_str(), "blog");
    assert_eq!(spec.placeholders.len(), 1);
    assert!(format!("{spec:?}").contains("SitemapSourceSpec"));
}
