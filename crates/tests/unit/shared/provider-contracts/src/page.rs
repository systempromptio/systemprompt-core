//! Coverage for `PageContext` accessors and `PageDataProvider` defaults.

use async_trait::async_trait;
use serde_json::{Value, json};
use systemprompt_identifiers::LocaleCode;
use systemprompt_provider_contracts::{PageContext, PageDataProvider, ProviderResult};

use crate::support::web_config;

#[test]
fn new_defaults_locale_to_web_config_default() {
    let wc = web_config();
    let cc: &(dyn std::any::Any + Send + Sync) = &();
    let pool: &(dyn std::any::Any + Send + Sync) = &();
    let ctx = PageContext::new("home", &wc, cc, pool);

    assert_eq!(ctx.page_type, "home");
    assert_eq!(ctx.locale.as_str(), "en");
    assert!(ctx.content_item().is_none());
    assert!(ctx.all_items().is_none());
}

#[test]
fn with_locale_overrides() {
    let wc = web_config();
    let cc: &(dyn std::any::Any + Send + Sync) = &();
    let pool: &(dyn std::any::Any + Send + Sync) = &();
    let locale = LocaleCode::new("de");
    let ctx = PageContext::new("home", &wc, cc, pool).with_locale(&locale);
    assert_eq!(ctx.locale.as_str(), "de");
}

#[test]
fn with_content_item_and_all_items() {
    let wc = web_config();
    let cc: &(dyn std::any::Any + Send + Sync) = &();
    let pool: &(dyn std::any::Any + Send + Sync) = &();
    let item = json!({"slug": "x"});
    let items = vec![json!({}), json!({})];
    let ctx = PageContext::new("blog", &wc, cc, pool)
        .with_content_item(&item)
        .with_all_items(&items);

    assert_eq!(ctx.content_item().unwrap()["slug"], "x");
    assert_eq!(ctx.all_items().unwrap().len(), 2);
}

#[test]
fn content_config_and_db_pool_downcast() {
    let wc = web_config();
    let cc: &(dyn std::any::Any + Send + Sync) = &"cfg".to_string();
    let pool: &(dyn std::any::Any + Send + Sync) = &123u32;
    let ctx = PageContext::new("home", &wc, cc, pool);

    assert_eq!(ctx.content_config::<String>(), Some(&"cfg".to_string()));
    assert!(ctx.content_config::<u32>().is_none());
    assert_eq!(ctx.db_pool::<u32>(), Some(&123u32));
    assert!(ctx.db_pool::<String>().is_none());
}

#[test]
fn debug_reports_item_presence_and_count() {
    let wc = web_config();
    let cc: &(dyn std::any::Any + Send + Sync) = &();
    let pool: &(dyn std::any::Any + Send + Sync) = &();
    let item = json!({});
    let items = vec![json!({})];
    let ctx = PageContext::new("home", &wc, cc, pool)
        .with_content_item(&item)
        .with_all_items(&items);

    let dbg = format!("{ctx:?}");
    assert!(dbg.contains("PageContext"));
    assert!(dbg.contains("content_item: true"));
    assert!(dbg.contains("all_items_count: Some(1)"));
}

struct MinimalProvider;

#[async_trait]
impl PageDataProvider for MinimalProvider {
    fn provider_id(&self) -> &'static str {
        "minimal"
    }

    async fn provide_page_data(&self, ctx: &PageContext<'_>) -> ProviderResult<Value> {
        Ok(json!({"page_type": ctx.page_type}))
    }
}

#[test]
fn trait_defaults() {
    let p = MinimalProvider;
    assert_eq!(p.provider_id(), "minimal");
    assert!(p.applies_to_pages().is_empty());
    assert_eq!(p.priority(), 100);
}

#[tokio::test]
async fn provide_page_data_reads_context() {
    let wc = web_config();
    let cc: &(dyn std::any::Any + Send + Sync) = &();
    let pool: &(dyn std::any::Any + Send + Sync) = &();
    let ctx = PageContext::new("about", &wc, cc, pool);
    let data = MinimalProvider.provide_page_data(&ctx).await.unwrap();
    assert_eq!(data["page_type"], "about");
}
