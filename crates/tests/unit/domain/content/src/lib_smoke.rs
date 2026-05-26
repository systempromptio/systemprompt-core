//! Smoke tests for content crate lib-level public exports.

use systemprompt_content::{
    DefaultBrandingProvider, DefaultHomepagePrerenderer, DefaultListBrandingProvider,
    ListItemsCardRenderer, default_branding_provider, default_homepage_prerenderer,
    default_list_branding_provider, default_list_items_renderer,
};
use systemprompt_provider_contracts::{ComponentRenderer, PageDataProvider, PagePrerenderer};

#[test]
fn test_default_branding_provider_id() {
    let provider = DefaultBrandingProvider;
    assert_eq!(provider.provider_id(), "default-branding");
}

#[test]
fn test_default_branding_provider_arc_factory() {
    let provider = default_branding_provider();
    assert_eq!(provider.provider_id(), "default-branding");
}

#[test]
fn test_default_branding_provider_default_priority() {
    let provider = DefaultBrandingProvider;
    assert_eq!(provider.priority(), 100);
}

#[test]
fn test_default_list_branding_provider_id() {
    let provider = DefaultListBrandingProvider;
    assert_eq!(provider.provider_id(), "default-list-branding");
}

#[test]
fn test_default_list_branding_provider_arc_factory() {
    let provider = default_list_branding_provider();
    assert_eq!(provider.provider_id(), "default-list-branding");
}

#[test]
fn test_list_items_card_renderer_component_id() {
    let renderer = ListItemsCardRenderer;
    assert_eq!(renderer.component_id(), "list-items-cards");
}

#[test]
fn test_list_items_card_renderer_variable_name() {
    let renderer = ListItemsCardRenderer;
    assert_eq!(renderer.variable_name(), "ITEMS");
}

#[test]
fn test_list_items_card_renderer_applies_to() {
    let renderer = ListItemsCardRenderer;
    let pages = renderer.applies_to();
    assert!(pages.iter().any(|p| p == "blog-list"));
    assert!(pages.iter().any(|p| p == "news-list"));
    assert!(pages.iter().any(|p| p == "pages-list"));
}

#[test]
fn test_list_items_card_renderer_priority() {
    let renderer = ListItemsCardRenderer;
    assert_eq!(renderer.priority(), 100);
}

#[test]
fn test_default_list_items_renderer_arc_factory() {
    let renderer = default_list_items_renderer();
    assert_eq!(renderer.component_id(), "list-items-cards");
}

#[test]
fn test_homepage_prerenderer_page_type() {
    let prerenderer = DefaultHomepagePrerenderer::new();
    assert_eq!(prerenderer.page_type(), "homepage");
}

#[test]
fn test_homepage_prerenderer_priority() {
    let prerenderer = DefaultHomepagePrerenderer::new();
    assert_eq!(prerenderer.priority(), 100);
}

#[test]
fn test_default_homepage_prerenderer_arc_factory() {
    let prerenderer = default_homepage_prerenderer();
    assert_eq!(prerenderer.page_type(), "homepage");
}

#[test]
fn test_default_branding_provider_clone_copy() {
    let p = DefaultBrandingProvider;
    let _p2 = p; // Copy
    let _p3 = p; // still usable
}

#[test]
fn test_default_list_branding_provider_default() {
    let _p: DefaultListBrandingProvider = DefaultListBrandingProvider::default();
}

#[test]
fn test_list_items_card_renderer_default() {
    let _r: ListItemsCardRenderer = ListItemsCardRenderer::default();
}

#[test]
fn test_homepage_prerenderer_default() {
    let p: DefaultHomepagePrerenderer = DefaultHomepagePrerenderer::default();
    assert_eq!(p.page_type(), "homepage");
}

#[test]
fn test_homepage_prerenderer_debug() {
    let p = DefaultHomepagePrerenderer::new();
    let d = format!("{:?}", p);
    assert!(d.contains("DefaultHomepagePrerenderer"));
}
