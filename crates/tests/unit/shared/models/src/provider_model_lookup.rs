//! The catalog lookup contract: `find_model` matches ids and aliases only.
//!
//! `ProviderModel::matches` deliberately ignores `upstream_model`, so every
//! caller must look a model up by the name the *client* asked for, never by
//! the name the upstream knows it as. Resolving the gateway's `model_limits`
//! by the upstream name silently returned `None` for both Vertex providers,
//! whose catalog ids differ from their upstream names, and the output-token
//! clamp and thinking budget quietly disappeared. These tests make the
//! asymmetry explicit rather than leaving it to be rediscovered.

use systemprompt_identifiers::{ModelId, ProviderId, SecretName};
use systemprompt_models::services::ai::ModelLimits;
use systemprompt_models::services::{ApiSurface, ProviderEntry, ProviderModel, WireProtocol};

const CATALOG_ID: &str = "vertex-gemini-2.5-pro";
const ALIAS: &str = "gemini-pro-latest";
const UPSTREAM: &str = "gemini-2.5-pro";

fn entry() -> ProviderEntry {
    ProviderEntry {
        name: ProviderId::new("vertex"),
        wire: WireProtocol::Gemini,
        surface: ApiSurface::Gemini,
        endpoint: "https://example.invalid/v1".to_owned(),
        api_key_secret: SecretName::new("vertex_api_key"),
        governance: Default::default(),
        extra_headers: std::collections::HashMap::new(),
        models: vec![ProviderModel {
            id: ModelId::new(CATALOG_ID),
            aliases: vec![ModelId::new(ALIAS)],
            governance: None,
            upstream_model: Some(UPSTREAM.to_owned()),
            pricing: Default::default(),
            capabilities: Default::default(),
            limits: ModelLimits {
                context_window: 1_048_576,
                max_output_tokens: 64,
                max_thinking_budget: Some(1024),
            },
        }],
    }
}

#[test]
fn find_model_matches_the_catalog_id() {
    let found = entry().find_model(CATALOG_ID).map(|m| m.limits);
    assert_eq!(
        found.map(|l| l.max_output_tokens),
        Some(64),
        "the catalog id is the key every caller has"
    );
}

#[test]
fn find_model_matches_an_alias() {
    assert!(
        entry().find_model(ALIAS).is_some(),
        "aliases are part of the documented match set"
    );
}

#[test]
fn find_model_does_not_match_the_upstream_name() {
    assert!(
        entry().find_model(UPSTREAM).is_none(),
        "matching the upstream name would make the lookup key ambiguous; \
         callers holding only the upstream name must not silently get limits"
    );
}

#[test]
fn upstream_model_for_translates_the_catalog_id() {
    let e = entry();
    assert_eq!(e.upstream_model_for(None, CATALOG_ID), UPSTREAM);
    assert_eq!(
        e.upstream_model_for(Some("route-override"), CATALOG_ID),
        "route-override"
    );
}
