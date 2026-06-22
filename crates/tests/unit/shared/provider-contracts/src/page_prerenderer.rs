//! Coverage for `PagePrepareContext`, `PageRenderSpec`, and the
//! `PagePrerenderer` trait defaults.

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use serde_json::json;
use systemprompt_identifiers::LocaleCode;
use systemprompt_provider_contracts::{
    PagePrepareContext, PagePrerenderer, PageRenderSpec, ProviderResult,
};

use crate::support::web_config;

#[test]
fn new_defaults_locale_and_exposes_dist_dir() {
    let wc = web_config();
    let cc: &(dyn std::any::Any + Send + Sync) = &();
    let pool: &(dyn std::any::Any + Send + Sync) = &();
    let dist = Path::new("/tmp/dist");
    let ctx = PagePrepareContext::new(&wc, cc, pool, dist);

    assert_eq!(ctx.locale.as_str(), "en");
    assert_eq!(ctx.dist_dir(), Path::new("/tmp/dist"));
}

#[test]
fn with_locale_overrides() {
    let wc = web_config();
    let cc: &(dyn std::any::Any + Send + Sync) = &();
    let pool: &(dyn std::any::Any + Send + Sync) = &();
    let dist = Path::new("/tmp/dist");
    let locale = LocaleCode::new("fr");
    let ctx = PagePrepareContext::new(&wc, cc, pool, dist).with_locale(&locale);
    assert_eq!(ctx.locale.as_str(), "fr");
}

#[test]
fn content_config_and_db_pool_downcast() {
    let wc = web_config();
    let cc: &(dyn std::any::Any + Send + Sync) = &7i64;
    let pool: &(dyn std::any::Any + Send + Sync) = &"pool".to_string();
    let dist = Path::new("/tmp/dist");
    let ctx = PagePrepareContext::new(&wc, cc, pool, dist);

    assert_eq!(ctx.content_config::<i64>(), Some(&7i64));
    assert!(ctx.content_config::<u8>().is_none());
    assert_eq!(ctx.db_pool::<String>(), Some(&"pool".to_string()));
    assert!(ctx.db_pool::<i64>().is_none());
}

#[test]
fn context_is_debug() {
    let wc = web_config();
    let cc: &(dyn std::any::Any + Send + Sync) = &();
    let pool: &(dyn std::any::Any + Send + Sync) = &();
    let dist = Path::new("/tmp/dist");
    let ctx = PagePrepareContext::new(&wc, cc, pool, dist);
    assert!(format!("{ctx:?}").contains("PagePrepareContext"));
}

#[test]
fn render_spec_new_assigns_fields() {
    let spec = PageRenderSpec::new("home.hbs", json!({"k": 1}), "/out/index.html");
    assert_eq!(spec.template_name, "home.hbs");
    assert_eq!(spec.base_data["k"], 1);
    assert_eq!(spec.output_path, PathBuf::from("/out/index.html"));
}

struct MinimalPrerenderer;

#[async_trait]
impl PagePrerenderer for MinimalPrerenderer {
    fn page_type(&self) -> &str {
        "home"
    }

    async fn prepare(
        &self,
        ctx: &PagePrepareContext<'_>,
    ) -> ProviderResult<Option<PageRenderSpec>> {
        Ok(Some(PageRenderSpec::new(
            "home.hbs",
            json!({"locale": ctx.locale.as_str()}),
            "index.html",
        )))
    }
}

#[test]
fn trait_default_priority() {
    let p = MinimalPrerenderer;
    assert_eq!(p.page_type(), "home");
    assert_eq!(p.priority(), 100);
}

#[tokio::test]
async fn prepare_returns_spec() {
    let wc = web_config();
    let cc: &(dyn std::any::Any + Send + Sync) = &();
    let pool: &(dyn std::any::Any + Send + Sync) = &();
    let dist = Path::new("/tmp/dist");
    let ctx = PagePrepareContext::new(&wc, cc, pool, dist);

    let spec = MinimalPrerenderer.prepare(&ctx).await.unwrap().unwrap();
    assert_eq!(spec.template_name, "home.hbs");
    assert_eq!(spec.base_data["locale"], "en");
}
