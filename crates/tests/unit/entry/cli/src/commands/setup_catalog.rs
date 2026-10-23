//! Tests for `admin::setup::catalog` — the catalog the setup wizard and
//! cloud-init seed from the supplied AI keys.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::admin::setup::catalog::{SeedRequest, first_model_on, seed_document};
use systemprompt_manifest::services::catalog::CatalogContext;

fn seed(
    present: &[&str],
    primary: Option<&str>,
) -> systemprompt_manifest::services::catalog::CatalogDocument {
    seed_document(SeedRequest {
        present,
        primary,
        gateway_enabled: true,
    })
    .expect("seed resolves")
}

#[test]
fn only_supplied_vendors_are_kept() {
    let doc = seed(&["openai"], Some("openai"));
    let providers: Vec<&str> = doc.providers.iter().map(|(id, _)| id.as_str()).collect();
    assert_eq!(providers, ["openai"]);
    assert!(doc.credentials.get("openai").is_some());
    assert!(doc.credentials.get("anthropic").is_none());
    assert!(doc.deployments.iter().all(|(_, targets)| {
        targets
            .iter()
            .all(|t| t.endpoint().provider.as_str() == "openai")
    }));
}

#[test]
fn default_model_is_served_by_the_primary_vendor() {
    let doc = seed(&["anthropic", "openai", "gemini"], Some("gemini"));
    let model = doc
        .gateway
        .as_ref()
        .and_then(|g| g.default_model.clone())
        .expect("default model");
    assert_eq!(first_model_on(&doc, "gemini"), Some(model.clone()));
    let catalog = doc.resolve(&CatalogContext::default()).expect("resolves");
    assert_eq!(
        catalog
            .enabled_hops(model.as_str())
            .next()
            .unwrap()
            .provider
            .as_str(),
        "gemini"
    );
}

#[test]
fn aliases_whose_target_is_gone_are_dropped() {
    let doc = seed(&["openai"], Some("openai"));
    let routes = &doc.gateway.as_ref().unwrap().routes;
    assert!(routes.iter().all(|r| r.alias.is_none()), "{routes:?}");
    let with_anthropic = seed(&["anthropic"], Some("anthropic"));
    assert!(
        with_anthropic
            .gateway
            .unwrap()
            .routes
            .iter()
            .any(|r| r.alias.is_some())
    );
}

#[test]
fn seeded_gateway_is_enabled_and_priced() {
    let doc = seed(&["anthropic", "openai", "gemini"], Some("anthropic"));
    assert!(doc.gateway.as_ref().unwrap().enabled);
    let catalog = doc
        .resolve(&CatalogContext::default())
        .expect("every hop is priced");
    assert!(catalog.gateway_enabled());
}
