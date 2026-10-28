//! Unit tests for the advertised model catalog behind `/v1/bridge/profile` and
//! `/v1/models`. Both list every advertised model (`clients` not empty) that a
//! configured credential can serve; a model with `clients: []` still routes but
//! is never listed, and a platform credential whose secret is absent, or a
//! personal credential the caller stored no key for, withholds its models while
//! `providers` still reports the provider with the reason.
//! `families_from_header` validates the advisory `x-inference-protocol` header.

use std::collections::BTreeSet;

use axum::http::HeaderMap;
use systemprompt_api::error::ApiHttpError;
use systemprompt_api::routes::gateway::bridge::provider_health;
use systemprompt_api::routes::gateway::models::{families_from_header, model_entries};
use systemprompt_catalog_fixtures::{
    CatalogFixture, claude_model, model, personal, platform, priced,
};
use systemprompt_identifiers::headers::INFERENCE_PROTOCOL;
use systemprompt_identifiers::{ModelId, TenantId};
use systemprompt_manifest::bridge_profile::{BridgeProfileParams, CredentialFacts, build};
use systemprompt_manifest::services::catalog::{EndpointConfig, ProviderConfig, ResolvedCatalog};
use systemprompt_models::api::ErrorCode;
use systemprompt_models::bridge::profile::BridgeProfileResponse;
use systemprompt_models::providers::ClientFamily;
use systemprompt_wire::WireProtocol;

fn catalog() -> ResolvedCatalog {
    CatalogFixture::new()
        .credential("anthropic", platform("anthropic"))
        .credential("openai", platform("openai"))
        .credential("go", personal("Go"))
        .provider("anthropic", ProviderConfig::new("anthropic"))
        .provider("openai", ProviderConfig::new("openai"))
        .provider("go", ProviderConfig::new("go"))
        .endpoint(
            "anthropic",
            "api",
            EndpointConfig::new(WireProtocol::Anthropic, "https://a.invalid/v1"),
        )
        .endpoint(
            "openai",
            "api",
            EndpointConfig::new(WireProtocol::OpenAiChat, "https://o.invalid/v1"),
        )
        .endpoint(
            "go",
            "chat",
            EndpointConfig::new(WireProtocol::OpenAiChat, "https://g.invalid/v1"),
        )
        .model("claude-sonnet-5", claude_model(1_000_000))
        .model(
            "claude-internal",
            model(&[], 200_000, priced(1.0, 2.0, 0.1)),
        )
        .model(
            "gpt-5",
            model(&[ClientFamily::OpenAi], 400_000, priced(1.0, 2.0, 0.1)),
        )
        .model(
            "go-kimi",
            model(&[ClientFamily::OpenAi], 0, priced(1.0, 2.0, 0.1)),
        )
        .deploy("claude-sonnet-5", ["anthropic/api"])
        .deploy("claude-internal", ["anthropic/api"])
        .deploy("gpt-5", ["openai/api"])
        .deploy("go-kimi", ["go/chat"])
        .default_model("claude-sonnet-5")
        .build()
}

fn profile(catalog: &ResolvedCatalog, secrets: &[&str], keys: &[&str]) -> BridgeProfileResponse {
    let present = |name: &str| secrets.contains(&name);
    let personal_keys: BTreeSet<String> = keys.iter().map(|k| (*k).to_owned()).collect();
    build(
        BridgeProfileParams {
            inference_gateway_base_url: "https://gw.invalid/v1".to_owned(),
            auth_scheme: "bearer".to_owned(),
            organization_uuid: None,
            catalog,
        },
        &CredentialFacts {
            secret_present: &present,
            personal_keys: &personal_keys,
        },
    )
}

#[test]
fn the_profile_lists_advertised_servable_models_only() {
    let c = catalog();
    let p = profile(&c, &["anthropic", "openai"], &[]);
    assert_eq!(
        p.models,
        vec![ModelId::new("claude-sonnet-5"), ModelId::new("gpt-5")]
    );
    assert_eq!(p.default_model.as_deref(), Some("claude-sonnet-5[1m]"));
}

#[test]
fn a_personal_credential_serves_only_a_caller_who_stored_a_key() {
    let c = catalog();
    assert!(
        !profile(&c, &["anthropic"], &[])
            .models
            .contains(&ModelId::new("go-kimi"))
    );
    assert!(
        profile(&c, &["anthropic"], &["go"])
            .models
            .contains(&ModelId::new("go-kimi"))
    );
}

#[test]
fn a_missing_secret_withholds_models_but_reports_the_provider() {
    let c = catalog();
    let p = profile(&c, &["anthropic"], &[]);
    assert!(!p.models.contains(&ModelId::new("gpt-5")));
    let openai = p
        .providers
        .iter()
        .find(|h| h.name == "openai")
        .expect("provider still reported");
    assert!(!openai.configured);
    assert!(
        openai
            .config_issue
            .as_deref()
            .is_some_and(|m| m.contains("openai"))
    );
    assert_eq!(openai.clients, vec![ClientFamily::OpenAi]);
}

#[test]
fn provider_health_carries_credential_and_clients() {
    let c = catalog();
    let present = |_: &str| true;
    let keys = BTreeSet::new();
    let health = provider_health(
        &c,
        &CredentialFacts {
            secret_present: &present,
            personal_keys: &keys,
        },
    );
    let anthropic = health
        .iter()
        .find(|h| h.name == "anthropic")
        .expect("listed");
    assert_eq!(anthropic.credential, "anthropic");
    assert_eq!(anthropic.models, vec!["claude-sonnet-5".to_owned()]);
    assert!(!anthropic.models.contains(&"claude-internal".to_owned()));
    let go = health.iter().find(|h| h.name == "go").expect("listed");
    assert!(go.personal_key && !go.configured);
}

#[test]
fn model_entries_list_every_advertised_configured_model() {
    let c = catalog();
    let ids: Vec<String> = model_entries(&c, |name| name == "anthropic")
        .into_iter()
        .map(|e| e.id)
        .collect();
    assert_eq!(
        ids,
        vec!["claude-sonnet-5".to_owned(), "go-kimi".to_owned()]
    );
    let entry = model_entries(&c, |_| true)
        .into_iter()
        .find(|e| e.id == "claude-sonnet-5")
        .expect("listed");
    assert_eq!(entry.max_input_tokens, Some(1_000_000));
}

#[test]
fn header_helper_parses_client_families_and_defaults_to_empty() {
    assert!(
        families_from_header(&HeaderMap::new())
            .expect("ok")
            .is_empty()
    );
    let mut h = HeaderMap::new();
    h.insert(INFERENCE_PROTOCOL, "claude, openai".parse().unwrap());
    assert_eq!(
        families_from_header(&h).expect("ok"),
        vec![ClientFamily::Claude, ClientFamily::OpenAi]
    );
}

#[test]
fn header_helper_rejects_garbage() {
    let mut h = HeaderMap::new();
    h.insert(INFERENCE_PROTOCOL, "anthropic".parse().unwrap());
    let err = ApiHttpError::from(families_from_header(&h).expect_err("not a family")).into_inner();
    assert_eq!(err.code, ErrorCode::BadRequest);
}

#[test]
fn organization_uuid_peels_local_prefix_from_uuid_tenant() {
    let tenant = TenantId::new("local_6fa459ea-ee8a-3ca4-894e-db77e160355e");
    assert_eq!(
        tenant.organization_uuid(),
        "6fa459ea-ee8a-3ca4-894e-db77e160355e"
    );
}

#[test]
fn organization_uuid_normalizes_bare_uuid_to_lowercase_canonical() {
    let tenant = TenantId::new("6FA459EA-EE8A-3CA4-894E-DB77E160355E");
    assert_eq!(
        tenant.organization_uuid(),
        "6fa459ea-ee8a-3ca4-894e-db77e160355e"
    );
}

#[test]
fn organization_uuid_derives_stable_v5_for_non_uuid_tenant() {
    let tenant = TenantId::new("local_acme-corp");
    let got = tenant.organization_uuid();
    assert_eq!(got.len(), 36, "{got}");
    assert_eq!(got.as_bytes()[14], b'5', "expected a v5 UUID: {got}");
    assert_eq!(
        got,
        tenant.organization_uuid(),
        "derivation must be deterministic"
    );
    assert_ne!(
        got,
        TenantId::new("local_other-corp").organization_uuid(),
        "distinct tenants must not collide"
    );
}
