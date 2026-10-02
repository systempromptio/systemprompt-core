//! Coverage for `PagePrepareContext`, `PageRenderSpec`, and the
//! `PagePrerenderer` trait defaults.

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use serde_json::json;
use systemprompt_identifiers::LocaleCode;
use systemprompt_provider_contracts::{
    Dependencies, PagePrepareContext, PagePrerenderer, PageRenderSpec, ProviderResult,
};

use crate::support::web_config;

#[test]
fn new_defaults_locale_and_exposes_dist_dir() {
    let wc = web_config();
    let deps = Dependencies::new();
    let dist = Path::new("/tmp/dist");
    let ctx = PagePrepareContext::new(&wc, &deps, dist);

    assert_eq!(ctx.locale.as_str(), "en");
    assert_eq!(ctx.dist_dir(), Path::new("/tmp/dist"));
}

#[test]
fn with_locale_overrides() {
    let wc = web_config();
    let deps = Dependencies::new();
    let dist = Path::new("/tmp/dist");
    let locale = LocaleCode::try_new("fr").expect("valid LocaleCode");
    let ctx = PagePrepareContext::new(&wc, &deps, dist).with_locale(&locale);
    assert_eq!(ctx.locale.as_str(), "fr");
}

#[test]
fn get_returns_inserted_dependencies_and_names_missing_ones() {
    let wc = web_config();
    let deps = Dependencies::new().with(7i64).with("pool".to_string());
    let dist = Path::new("/tmp/dist");
    let ctx = PagePrepareContext::new(&wc, &deps, dist);

    assert_eq!(ctx.get::<i64>(), Ok(&7i64));
    assert_eq!(ctx.get::<String>(), Ok(&"pool".to_string()));
    assert_eq!(ctx.get::<u8>().unwrap_err().type_name(), "u8");
}

#[test]
fn context_is_debug() {
    let wc = web_config();
    let deps = Dependencies::new();
    let dist = Path::new("/tmp/dist");
    let ctx = PagePrepareContext::new(&wc, &deps, dist);
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
    let deps = Dependencies::new();
    let dist = Path::new("/tmp/dist");
    let ctx = PagePrepareContext::new(&wc, &deps, dist);

    let spec = MinimalPrerenderer.prepare(&ctx).await.unwrap().unwrap();
    assert_eq!(spec.template_name, "home.hbs");
    assert_eq!(spec.base_data["locale"], "en");
}
