use std::path::PathBuf;

use serde_json::json;
use systemprompt_identifiers::SourceId;
use systemprompt_provider_contracts::{
    ContentDataContext, ExtendedData, FrontmatterContext, PartialSource, PartialTemplate,
    PlaceholderMapping, RenderedComponent, RssFeedContext, RssFeedItem, RssFeedMetadata,
    RssFeedSpec, SitemapAlternate, SitemapContext, SitemapSourceSpec, SitemapUrlEntry,
};

#[test]
fn partial_template_embedded_and_file_constructors() {
    let t = PartialTemplate::embedded("header", "<h1>x</h1>");
    assert_eq!(t.name, "header");
    match t.source {
        PartialSource::Embedded(s) => assert_eq!(s, "<h1>x</h1>"),
        PartialSource::File(_) => panic!("expected embedded"),
    }

    let t = PartialTemplate::file("footer", "/tmp/foo.html");
    assert_eq!(t.name, "footer");
    match t.source {
        PartialSource::File(p) => assert_eq!(p, PathBuf::from("/tmp/foo.html")),
        PartialSource::Embedded(_) => panic!("expected file"),
    }
}

#[test]
fn rendered_component_new_assigns_fields() {
    let r = RenderedComponent::new("variable", "<div/>");
    assert_eq!(r.variable_name, "variable");
    assert_eq!(r.html, "<div/>");
}

#[test]
fn frontmatter_context_accessors_and_db_pool_downcast() {
    let raw = serde_yaml::Value::String("hello".to_owned());
    let pool_value: i64 = 42;
    let pool: &(dyn std::any::Any + Send + Sync) = &pool_value;

    let ctx = FrontmatterContext::new("cid", "slug-1", "blog", &raw, pool);
    assert_eq!(ctx.content_id(), "cid");
    assert_eq!(ctx.slug(), "slug-1");
    assert_eq!(ctx.source_name(), "blog");
    assert!(matches!(ctx.raw_frontmatter(), serde_yaml::Value::String(s) if s == "hello"));
    assert_eq!(ctx.db_pool::<i64>().copied(), Some(42));
    assert!(ctx.db_pool::<String>().is_none());
}

#[test]
fn frontmatter_context_debug_does_not_leak_pool() {
    let raw = serde_yaml::Value::Null;
    let pool_value: i64 = 0;
    let pool: &(dyn std::any::Any + Send + Sync) = &pool_value;
    let ctx = FrontmatterContext::new("cid", "slug", "src", &raw, pool);
    let dbg = format!("{ctx:?}");
    assert!(dbg.contains("FrontmatterContext"));
    assert!(dbg.contains("<dyn Any>"));
}

#[test]
fn sitemap_context_holds_borrows() {
    let ctx = SitemapContext {
        base_url: "https://example.com",
        source_name: "blog",
    };
    assert_eq!(ctx.base_url, "https://example.com");
    assert_eq!(ctx.source_name, "blog");
}

#[test]
fn content_data_context_accessors_and_downcast() {
    let pool_value: u32 = 7;
    let pool: &(dyn std::any::Any + Send + Sync) = &pool_value;
    let ctx = ContentDataContext::new("cid-1", "blog", pool);
    assert_eq!(ctx.content_id(), "cid-1");
    assert_eq!(ctx.source_name(), "blog");
    assert_eq!(ctx.db_pool::<u32>().copied(), Some(7));
    assert!(ctx.db_pool::<i64>().is_none());

    let dbg = format!("{ctx:?}");
    assert!(dbg.contains("ContentDataContext"));
    assert!(dbg.contains("<dyn Any>"));
}

#[test]
fn extended_data_constructors_set_priority() {
    let d = ExtendedData::new(json!({"k": "v"}));
    assert_eq!(d.priority, 100);
    assert_eq!(d.variables["k"], "v");

    let d = ExtendedData::with_priority(json!({}), 25);
    assert_eq!(d.priority, 25);
}

#[test]
fn page_render_spec_new_assigns_fields() {
    let spec = systemprompt_provider_contracts::PageRenderSpec::new(
        "home",
        json!({"key": "value"}),
        std::path::PathBuf::from("/out/index.html"),
    );
    assert_eq!(spec.template_name, "home");
    assert_eq!(spec.output_path, std::path::PathBuf::from("/out/index.html"));
    assert_eq!(spec.base_data["key"], "value");
}

#[test]
fn template_definition_constructors_chain_builders() {
    use systemprompt_provider_contracts::{TemplateDefinition, TemplateSource};

    let t = TemplateDefinition::embedded("page", "<html/>");
    assert_eq!(t.name, "page");
    assert_eq!(t.priority, 100);
    assert!(t.content_types.is_empty());
    assert!(matches!(t.source, TemplateSource::Embedded(s) if s == "<html/>"));

    let t = TemplateDefinition::file("from-file", "/tmp/x.hbs");
    assert!(matches!(t.source, TemplateSource::File(p) if p == PathBuf::from("/tmp/x.hbs")));

    let t = TemplateDefinition::directory("dir", "/tmp/dir");
    assert!(matches!(t.source, TemplateSource::Directory(p) if p == PathBuf::from("/tmp/dir")));

    let t = TemplateDefinition::embedded("p", "x")
        .with_priority(50)
        .for_content_type("post")
        .for_content_types(vec!["page".to_owned()]);
    assert_eq!(t.priority, 50);
    assert_eq!(t.content_types, vec!["page".to_owned()]);
}

#[test]
fn rss_feed_structs_are_constructible() {
    let ctx = RssFeedContext {
        base_url: "https://example.com",
        source_name: "blog",
    };
    assert_eq!(ctx.base_url, "https://example.com");

    let meta = RssFeedMetadata {
        title: "T".to_owned(),
        link: "https://x".to_owned(),
        description: "d".to_owned(),
        language: Some("en".to_owned()),
    };
    assert_eq!(meta.language.as_deref(), Some("en"));

    let item = RssFeedItem {
        title: "Post".to_owned(),
        link: "https://x/post".to_owned(),
        description: "desc".to_owned(),
        pub_date: chrono::Utc::now(),
        guid: "guid".to_owned(),
        author: None,
    };
    assert!(item.author.is_none());

    let spec = RssFeedSpec {
        source_id: SourceId::new("blog"),
        max_items: 10,
        output_filename: "feed.xml".to_owned(),
    };
    assert_eq!(spec.max_items, 10);
}

#[test]
fn sitemap_structs_are_constructible() {
    let alt = SitemapAlternate {
        hreflang: systemprompt_identifiers::LocaleCode::new("en"),
        href: "https://example.com/en".to_owned(),
    };
    let entry = SitemapUrlEntry {
        loc: "https://example.com/posts/1".to_owned(),
        lastmod: "2025-01-01".to_owned(),
        changefreq: "daily".to_owned(),
        priority: 0.8,
        alternates: vec![alt.clone()],
    };
    assert_eq!(entry.alternates.len(), 1);
    assert_eq!(entry.alternates[0].href, alt.href);

    let spec = SitemapSourceSpec {
        source_id: SourceId::new("posts"),
        url_pattern: "/posts/{slug}".to_owned(),
        placeholders: vec![PlaceholderMapping {
            placeholder: "{slug}".to_owned(),
            field: "slug".to_owned(),
        }],
        priority: 0.5,
        changefreq: "weekly".to_owned(),
    };
    assert_eq!(spec.placeholders.len(), 1);
    assert_eq!(spec.placeholders[0].placeholder, "{slug}");
}
