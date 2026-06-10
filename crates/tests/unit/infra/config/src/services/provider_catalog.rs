#![allow(clippy::all)]

use std::collections::HashMap;

use systemprompt_config::{ConfigError, ModelSpec, ProviderCatalogService, ProviderSpec};
use systemprompt_identifiers::{ModelId, ProviderId, SecretName};
use systemprompt_models::profile::{ApiSurface, ProviderRegistry, WireProtocol};

fn spec(name: &str, endpoint: &str) -> ProviderSpec {
    ProviderSpec {
        name: ProviderId::new(name),
        wire: WireProtocol::Anthropic,
        surface: ApiSurface::Backend,
        endpoint: endpoint.to_owned(),
        api_key_secret: SecretName::new("EXAMPLE_API_KEY"),
        extra_headers: HashMap::new(),
    }
}

fn model_spec(provider: &str, id: &str) -> ModelSpec {
    ModelSpec {
        provider: ProviderId::new(provider),
        id: ModelId::new(id),
        aliases: vec![ModelId::new("alias-1")],
        upstream_model: Some("vendor-name".to_owned()),
    }
}

fn registry_with_provider_and_model() -> ProviderRegistry {
    let mut registry = ProviderRegistry::default();
    ProviderCatalogService::upsert_provider(&mut registry, spec("minimax", "https://a.example"));
    ProviderCatalogService::upsert_model(&mut registry, model_spec("minimax", "m2")).unwrap();
    registry
}

#[test]
fn upsert_provider_adds_entry() {
    let mut registry = ProviderRegistry::default();

    ProviderCatalogService::upsert_provider(&mut registry, spec("minimax", "https://a.example"));

    assert_eq!(registry.providers.len(), 1);
    let entry = &registry.providers[0];
    assert_eq!(entry.name.as_str(), "minimax");
    assert_eq!(entry.endpoint, "https://a.example");
    assert!(entry.models.is_empty());
}

#[test]
fn upsert_provider_replaces_in_place_and_preserves_models() {
    let mut registry = registry_with_provider_and_model();

    ProviderCatalogService::upsert_provider(&mut registry, spec("minimax", "https://b.example"));

    assert_eq!(registry.providers.len(), 1);
    let entry = &registry.providers[0];
    assert_eq!(entry.endpoint, "https://b.example");
    assert_eq!(entry.models.len(), 1);
    assert_eq!(entry.models[0].id.as_str(), "m2");
}

#[test]
fn remove_provider_deletes_entry() {
    let mut registry = registry_with_provider_and_model();

    ProviderCatalogService::remove_provider(&mut registry, &ProviderId::new("minimax")).unwrap();

    assert!(registry.providers.is_empty());
}

#[test]
fn remove_provider_unknown_name_errors() {
    let mut registry = ProviderRegistry::default();

    let err =
        ProviderCatalogService::remove_provider(&mut registry, &ProviderId::new("ghost"))
            .unwrap_err();

    assert!(matches!(err, ConfigError::ProviderNotFound { .. }));
    assert_eq!(err.to_string(), "No provider named ghost");
}

#[test]
fn upsert_model_adds_model_with_aliases_and_upstream() {
    let registry = registry_with_provider_and_model();

    let model = &registry.providers[0].models[0];
    assert_eq!(model.id.as_str(), "m2");
    assert_eq!(model.aliases.len(), 1);
    assert_eq!(model.aliases[0].as_str(), "alias-1");
    assert_eq!(model.upstream_model.as_deref(), Some("vendor-name"));
}

#[test]
fn upsert_model_replaces_existing_id() {
    let mut registry = registry_with_provider_and_model();
    let replacement = ModelSpec {
        provider: ProviderId::new("minimax"),
        id: ModelId::new("m2"),
        aliases: Vec::new(),
        upstream_model: None,
    };

    ProviderCatalogService::upsert_model(&mut registry, replacement).unwrap();

    let models = &registry.providers[0].models;
    assert_eq!(models.len(), 1);
    assert!(models[0].aliases.is_empty());
    assert_eq!(models[0].upstream_model, None);
}

#[test]
fn upsert_model_unknown_provider_errors() {
    let mut registry = ProviderRegistry::default();

    let err =
        ProviderCatalogService::upsert_model(&mut registry, model_spec("ghost", "m")).unwrap_err();

    assert_eq!(err.to_string(), "No provider named ghost");
}

#[test]
fn remove_model_deletes_model() {
    let mut registry = registry_with_provider_and_model();

    ProviderCatalogService::remove_model(
        &mut registry,
        &ProviderId::new("minimax"),
        &ModelId::new("m2"),
    )
    .unwrap();

    assert!(registry.providers[0].models.is_empty());
}

#[test]
fn remove_model_unknown_id_errors() {
    let mut registry = registry_with_provider_and_model();

    let err = ProviderCatalogService::remove_model(
        &mut registry,
        &ProviderId::new("minimax"),
        &ModelId::new("nope"),
    )
    .unwrap_err();

    assert!(matches!(err, ConfigError::ModelNotFound { .. }));
    assert_eq!(
        err.to_string(),
        "No model with id nope under provider minimax"
    );
}

#[test]
fn remove_model_unknown_provider_errors() {
    let mut registry = ProviderRegistry::default();

    let err = ProviderCatalogService::remove_model(
        &mut registry,
        &ProviderId::new("ghost"),
        &ModelId::new("m"),
    )
    .unwrap_err();

    assert!(matches!(err, ConfigError::ProviderNotFound { .. }));
}
