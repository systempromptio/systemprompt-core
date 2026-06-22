//! Coverage for `ComponentContext` constructors and `ComponentRenderer`
//! defaults.

use async_trait::async_trait;
use serde_json::json;
use systemprompt_provider_contracts::{
    ComponentContext, ComponentRenderer, ProviderResult, RenderedComponent,
};

use crate::support::web_config;

#[test]
fn for_page_has_no_item_or_list() {
    let wc = web_config();
    let ctx = ComponentContext::for_page(&wc);
    assert!(ctx.item.is_none());
    assert!(ctx.all_items.is_none());
    assert!(ctx.popular_ids.is_none());
}

#[test]
fn for_content_populates_all() {
    let wc = web_config();
    let item = json!({"id": "a"});
    let all = vec![json!({}), json!({})];
    let popular = vec!["a".to_string(), "b".to_string()];
    let ctx = ComponentContext::for_content(&wc, &item, &all, &popular);

    assert_eq!(ctx.item.unwrap()["id"], "a");
    assert_eq!(ctx.all_items.unwrap().len(), 2);
    assert_eq!(ctx.popular_ids.unwrap().len(), 2);
}

#[test]
fn for_list_sets_only_all_items() {
    let wc = web_config();
    let all = vec![json!({})];
    let ctx = ComponentContext::for_list(&wc, &all);

    assert!(ctx.item.is_none());
    assert_eq!(ctx.all_items.unwrap().len(), 1);
    assert!(ctx.popular_ids.is_none());
}

#[test]
fn context_is_debug() {
    let wc = web_config();
    let ctx = ComponentContext::for_page(&wc);
    assert!(format!("{ctx:?}").contains("ComponentContext"));
}

struct MinimalRenderer;

#[async_trait]
impl ComponentRenderer for MinimalRenderer {
    fn component_id(&self) -> &'static str {
        "min"
    }

    fn variable_name(&self) -> &'static str {
        "min_html"
    }

    async fn render(&self, _ctx: &ComponentContext<'_>) -> ProviderResult<RenderedComponent> {
        Ok(RenderedComponent::new("min_html", "<div></div>"))
    }
}

#[test]
fn trait_defaults() {
    let r = MinimalRenderer;
    assert_eq!(r.component_id(), "min");
    assert_eq!(r.variable_name(), "min_html");
    assert!(r.applies_to().is_empty());
    assert!(r.partial_template().is_none());
    assert_eq!(r.priority(), 100);
}

#[tokio::test]
async fn render_returns_named_component() {
    let wc = web_config();
    let ctx = ComponentContext::for_page(&wc);
    let out = MinimalRenderer.render(&ctx).await.unwrap();
    assert_eq!(out.variable_name, "min_html");
    assert_eq!(out.html, "<div></div>");
}
