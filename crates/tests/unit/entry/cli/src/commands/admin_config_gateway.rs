//! Tests for the `admin config gateway` mutators: enable state, policy-route
//! upsert/remove by id, aliases, and validation against the default catalog.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::admin::config::gateway::{
    RouteAddArgs, add_route, remove_route, set_enabled, spec_mut,
};
use systemprompt_manifest::services::catalog::{CatalogContext, CatalogDocument, DefaultCatalog};

fn document() -> CatalogDocument {
    let mut document = DefaultCatalog::document().expect("default catalog parses");
    document.gateway = None;
    document
}

fn route_args(
    id: &str,
    models: &[&str],
    deployments: &[&str],
    alias: Option<&str>,
) -> RouteAddArgs {
    RouteAddArgs {
        id: id.to_owned(),
        label: None,
        models: models.iter().map(|m| (*m).to_owned()).collect(),
        deployments: deployments.iter().map(|d| (*d).to_owned()).collect(),
        alias: alias.map(str::to_owned),
    }
}

fn resolves(document: &CatalogDocument) -> Result<(), String> {
    document
        .resolve(&CatalogContext::default())
        .map(drop)
        .map_err(|e| e.to_string())
}

#[test]
fn set_enabled_creates_spec_and_toggles_flag() {
    let mut doc = document();
    assert_eq!(
        set_enabled(spec_mut(&mut doc), true),
        "Gateway enabled = true"
    );
    assert!(doc.gateway.as_ref().unwrap().enabled);
    assert_eq!(
        set_enabled(spec_mut(&mut doc), false),
        "Gateway enabled = false"
    );
    assert!(!doc.gateway.as_ref().unwrap().enabled);
}

#[test]
fn add_route_upserts_by_declared_id() {
    let mut doc = document();
    let msg = add_route(
        spec_mut(&mut doc),
        &route_args("claude", &["claude-*"], &[], None),
    )
    .unwrap();
    assert_eq!(msg, "Route claude saved");
    add_route(
        spec_mut(&mut doc),
        &route_args("claude", &["claude-opus-*"], &[], None),
    )
    .unwrap();
    let routes = &doc.gateway.as_ref().unwrap().routes;
    assert_eq!(routes.len(), 1);
    assert_eq!(routes[0].models.0, vec!["claude-opus-*".to_owned()]);
    resolves(&doc).unwrap();
}

#[test]
fn remove_route_deletes_by_id_and_errors_when_absent() {
    let mut doc = document();
    add_route(
        spec_mut(&mut doc),
        &route_args("claude", &["claude-*"], &[], None),
    )
    .unwrap();
    add_route(
        spec_mut(&mut doc),
        &route_args("gpt", &["gpt-*"], &[], None),
    )
    .unwrap();
    assert_eq!(
        remove_route(spec_mut(&mut doc), "claude").unwrap(),
        "Route claude removed"
    );
    assert_eq!(doc.gateway.as_ref().unwrap().routes.len(), 1);
    let err = remove_route(spec_mut(&mut doc), "claude")
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("No route with id claude"),
        "unexpected error: {err}"
    );
}

#[test]
fn alias_route_resolves_to_its_model() {
    let mut doc = document();
    add_route(
        spec_mut(&mut doc),
        &route_args(
            "sonnet-latest",
            &[],
            &[],
            Some("sonnet-latest=claude-sonnet-5"),
        ),
    )
    .unwrap();
    resolves(&doc).unwrap();
    let err = add_route(
        spec_mut(&mut doc),
        &route_args("broken", &[], &[], Some("no-separator")),
    )
    .unwrap_err()
    .to_string();
    assert!(err.contains("REQUESTED=MODEL"), "unexpected error: {err}");
}

#[test]
fn route_pinning_an_undeployed_endpoint_does_not_resolve() {
    let mut doc = document();
    add_route(
        spec_mut(&mut doc),
        &route_args("pin", &["claude-*"], &["openai/api"], None),
    )
    .unwrap();
    let err = resolves(&doc).unwrap_err();
    assert!(err.contains("pin"), "unexpected error: {err}");
}

#[test]
fn route_matching_no_model_does_not_resolve() {
    let mut doc = document();
    add_route(
        spec_mut(&mut doc),
        &route_args("ghost", &["ghost-*"], &[], None),
    )
    .unwrap();
    let err = resolves(&doc).unwrap_err();
    assert!(err.contains("ghost"), "unexpected error: {err}");
}
