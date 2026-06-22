//! Coverage for `ExtenderContext`, its builder, and the
//! `TemplateDataExtender` trait defaults.

use async_trait::async_trait;
use serde_json::{Value, json};
use systemprompt_provider_contracts::{
    ExtendedData, ExtenderContext, ProviderResult, TemplateDataExtender,
};

use crate::support::web_config;

#[test]
fn builder_defaults_are_empty_strings() {
    let item = json!({"id": "a"});
    let all = vec![item.clone()];
    let cfg = serde_yaml::Value::Null;
    let wc = web_config();
    let pool: &(dyn std::any::Any + Send + Sync) = &7i32;

    let ctx = ExtenderContext::builder(&item, &all, &cfg, &wc, pool).build();

    assert_eq!(ctx.content_html, "");
    assert_eq!(ctx.url_pattern, "");
    assert_eq!(ctx.source_name, "");
    assert_eq!(ctx.all_items.len(), 1);
    assert_eq!(ctx.item["id"], "a");
}

#[test]
fn builder_with_setters_populate_fields() {
    let item = json!({});
    let all: Vec<Value> = vec![];
    let cfg = serde_yaml::Value::Null;
    let wc = web_config();
    let pool: &(dyn std::any::Any + Send + Sync) = &();

    let ctx = ExtenderContext::builder(&item, &all, &cfg, &wc, pool)
        .with_content_html("<p>hi</p>")
        .with_url_pattern("/blog/{slug}")
        .with_source_name("blog")
        .build();

    assert_eq!(ctx.content_html, "<p>hi</p>");
    assert_eq!(ctx.url_pattern, "/blog/{slug}");
    assert_eq!(ctx.source_name, "blog");
}

#[test]
fn db_pool_downcast_correct_and_wrong_type() {
    let item = json!({});
    let all: Vec<Value> = vec![];
    let cfg = serde_yaml::Value::Null;
    let wc = web_config();
    let pool: &(dyn std::any::Any + Send + Sync) = &99u64;

    let ctx = ExtenderContext::builder(&item, &all, &cfg, &wc, pool).build();

    assert_eq!(ctx.db_pool::<u64>(), Some(&99u64));
    assert!(ctx.db_pool::<String>().is_none());
}

#[test]
fn context_debug_summarizes_collections() {
    let item = json!({"k": 1});
    let all = vec![json!({}), json!({})];
    let cfg = serde_yaml::Value::Null;
    let wc = web_config();
    let pool: &(dyn std::any::Any + Send + Sync) = &();

    let ctx = ExtenderContext::builder(&item, &all, &cfg, &wc, pool)
        .with_content_html("abcde")
        .with_url_pattern("/x")
        .with_source_name("src")
        .build();

    let dbg = format!("{ctx:?}");
    assert!(dbg.contains("ExtenderContext"));
    assert!(dbg.contains("[2 items]"));
    assert!(dbg.contains("[5 chars]"));
    assert!(dbg.contains("<dyn Any>"));
}

#[test]
fn builder_debug_summarizes_collections() {
    let item = json!({});
    let all = vec![json!({})];
    let cfg = serde_yaml::Value::Null;
    let wc = web_config();
    let pool: &(dyn std::any::Any + Send + Sync) = &();

    let builder = ExtenderContext::builder(&item, &all, &cfg, &wc, pool).with_content_html("ab");
    let dbg = format!("{builder:?}");
    assert!(dbg.contains("ExtenderContextBuilder"));
    assert!(dbg.contains("[1 items]"));
    assert!(dbg.contains("[2 chars]"));
}

#[test]
fn extended_data_priority_default_and_override() {
    assert_eq!(ExtendedData::new(json!({})).priority, 100);
    assert_eq!(ExtendedData::with_priority(json!({}), 3).priority, 3);
}

struct MinimalExtender;

#[async_trait]
impl TemplateDataExtender for MinimalExtender {
    fn extender_id(&self) -> &str {
        "minimal"
    }

    async fn extend(&self, ctx: &ExtenderContext<'_>, data: &mut Value) -> ProviderResult<()> {
        data["source"] = json!(ctx.source_name);
        Ok(())
    }
}

#[test]
fn trait_defaults_applies_to_empty_priority_100() {
    let e = MinimalExtender;
    assert_eq!(e.extender_id(), "minimal");
    assert!(e.applies_to().is_empty());
    assert_eq!(e.priority(), 100);
}

#[tokio::test]
async fn extend_mutates_data_from_context() {
    let item = json!({});
    let all: Vec<Value> = vec![];
    let cfg = serde_yaml::Value::Null;
    let wc = web_config();
    let pool: &(dyn std::any::Any + Send + Sync) = &();
    let ctx = ExtenderContext::builder(&item, &all, &cfg, &wc, pool)
        .with_source_name("docs")
        .build();

    let mut data = json!({});
    MinimalExtender.extend(&ctx, &mut data).await.unwrap();
    assert_eq!(data["source"], "docs");
}
