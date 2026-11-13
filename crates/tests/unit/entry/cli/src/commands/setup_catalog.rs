//! Tests for `shared::provider_catalog` — the catalog the setup wizard and
//! cloud-init seed from the supplied AI keys.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::shared::provider_catalog::{
    SeedRequest, VERTEX, VertexSeed, add_vertex, first_model_on, seed_document,
};
use systemprompt_cli::admin::setup::secrets::{SecretsData, vertex};
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

#[test]
fn a_workload_identity_vertex_provider_resolves_without_a_secret() {
    let mut doc = seed(&["anthropic"], Some("anthropic"));
    add_vertex(
        &mut doc,
        VertexSeed::WorkloadIdentity {
            project: "acme-prod",
        },
    )
    .expect("vertex seed resolves");
    let credential = doc.credentials.get(VERTEX).expect("vertex credential");
    assert!(credential.secret.is_none());
    assert_eq!(
        credential
            .workload_identity
            .as_ref()
            .map(|w| w.project.as_str()),
        Some("acme-prod")
    );
    let provider = doc.providers.get(VERTEX).expect("vertex provider");
    assert_eq!(provider.endpoints.len(), 2);
}

#[test]
fn a_key_file_vertex_provider_names_the_vertex_secret() {
    let mut doc = seed(&["anthropic"], Some("anthropic"));
    add_vertex(&mut doc, VertexSeed::ServiceAccountKey).expect("vertex seed resolves");
    let credential = doc.credentials.get(VERTEX).expect("vertex credential");
    assert_eq!(credential.secret.as_ref().map(|s| s.as_str()), Some(VERTEX));
    assert!(credential.workload_identity.is_none());
}

#[test]
fn a_vertex_project_id_follows_googles_rules() {
    vertex::validate_project("acme-prod-1").expect("valid");
    for bad in [
        "short",
        "Acme-prod",
        "1acme-prod",
        "acme-prod-",
        "acme_prod",
        &"a".repeat(31),
    ] {
        assert!(vertex::validate_project(bad).is_err(), "{bad}");
    }
}

#[test]
fn a_vertex_project_is_carried_but_never_serialised() {
    let mut secrets = SecretsData::default();
    vertex::apply(&mut secrets, Some("acme-prod"), None).expect("applies");
    assert_eq!(
        secrets.vertex_seed(),
        Some(VertexSeed::WorkloadIdentity {
            project: "acme-prod"
        })
    );
    let json = serde_json::to_string(&secrets).expect("serialises");
    assert!(!json.contains("acme-prod"), "{json}");
    assert!(
        vertex::apply(
            &mut SecretsData::default(),
            Some("acme-prod"),
            Some(std::path::Path::new("key.json"))
        )
        .is_err()
    );
}

#[test]
fn a_vertex_key_file_must_be_a_service_account() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("key.json");
    std::fs::write(&path, "sk-not-a-service-account").expect("write");
    assert!(vertex::read_key_file(&path).is_err());
    assert!(vertex::read_key_file(&dir.path().join("missing.json")).is_err());
}
