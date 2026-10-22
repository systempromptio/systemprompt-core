#![allow(clippy::all)]

//! `ProviderCatalogService` over the four catalog registries: upserts keep
//! every field their spec leaves unset, an edit whose result does not resolve
//! is refused and leaves the document untouched, and the files round-trip.

use std::collections::{BTreeSet, HashMap};

use systemprompt_config::{
    ConfigError, CredentialSpec, EndpointSpec, ModelSpec, ProviderCatalogService, ProviderSpec,
    read_catalog, write_catalog,
};
use systemprompt_identifiers::{CredentialId, EndpointId, ModelId, ProviderId};
use systemprompt_manifest::services::catalog::{
    BetaSpec, CatalogContext, CatalogDocument, DeploymentTarget, EndpointRef,
};
use systemprompt_models::providers::ClientFamily;
use systemprompt_test_fixtures::catalog::{one_platform_provider, opencode_go, platform, priced};
use systemprompt_wire::WireProtocol;
use systemprompt_wire::anthropic::AnthropicBeta;

fn ctx() -> CatalogContext {
    CatalogContext::default()
}

fn document() -> CatalogDocument {
    one_platform_provider().document().clone()
}

fn provider_spec(id: &str, credential: &str) -> ProviderSpec {
    ProviderSpec {
        id: ProviderId::new(id),
        credential: CredentialId::new(credential),
        display_name: None,
        user_agent: None,
        extra_headers: HashMap::new(),
        enabled: None,
    }
}

fn deploy(endpoint: &str) -> Vec<DeploymentTarget> {
    vec![DeploymentTarget::Endpoint(
        EndpointRef::parse(endpoint).unwrap(),
    )]
}

fn model_spec(id: &str, endpoint: &str) -> ModelSpec {
    ModelSpec {
        id: ModelId::new(id),
        upstream_model: Some("vendor-name".to_owned()),
        display_name: None,
        clients: Some(vec![ClientFamily::Claude]),
        deployments: deploy(endpoint),
        pricing: Some(priced(1.0, 2.0, 0.1)),
    }
}

#[test]
fn a_credential_provider_endpoint_and_model_are_added_together() {
    let mut doc = document();
    ProviderCatalogService::upsert_credential(
        &mut doc,
        &ctx(),
        CredentialSpec {
            id: CredentialId::new("minimax"),
            entry: platform("minimax"),
        },
    )
    .unwrap();
    ProviderCatalogService::upsert_provider(&mut doc, &ctx(), provider_spec("minimax", "minimax"))
        .unwrap();
    ProviderCatalogService::upsert_endpoint(
        &mut doc,
        &ctx(),
        EndpointSpec {
            provider: ProviderId::new("minimax"),
            id: EndpointId::new("messages"),
            wire: WireProtocol::Anthropic,
            url: "https://a.example".to_owned(),
        },
    )
    .unwrap();
    let unpriced = ProviderCatalogService::upsert_model(
        &mut doc,
        &ctx(),
        ModelSpec {
            pricing: None,
            ..model_spec("m2", "minimax/messages")
        },
    )
    .unwrap_err();
    assert!(
        matches!(unpriced, ConfigError::CatalogRefused(_)),
        "a new model needs a price"
    );
    ProviderCatalogService::upsert_model(&mut doc, &ctx(), model_spec("m2", "minimax/messages"))
        .unwrap();
    let resolved = doc.resolve(&ctx()).unwrap();
    assert!(resolved.provider("minimax").is_some());
    assert_eq!(resolved.deployments("m2")[0].upstream_model, "vendor-name");
    assert!(
        resolved
            .endpoint(&EndpointRef::new("minimax", "messages"))
            .is_some()
    );
}

#[test]
fn a_model_upsert_takes_its_deployments_and_keeps_its_price() {
    let mut doc = document();
    let before = doc.models.models.get("claude-sonnet-5").unwrap().pricing;
    let spec = ModelSpec {
        upstream_model: None,
        pricing: None,
        ..model_spec("claude-sonnet-5", "anthropic/api")
    };
    ProviderCatalogService::upsert_model(&mut doc, &ctx(), spec).unwrap();
    let model = doc.models.models.get("claude-sonnet-5").unwrap();
    assert_eq!(
        format!("{:?}", model.pricing),
        format!("{before:?}"),
        "the spec carries no price, so none is lost"
    );
    assert_eq!(
        doc.deployments
            .deployments
            .get("claude-sonnet-5")
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn a_model_without_deployments_is_refused() {
    let mut doc = document();
    let spec = ModelSpec {
        deployments: Vec::new(),
        ..model_spec("m3", "anthropic/api")
    };
    let err = ProviderCatalogService::upsert_model(&mut doc, &ctx(), spec).unwrap_err();
    assert!(matches!(err, ConfigError::ModelWithoutDeployments { .. }));
}

#[test]
fn an_edit_that_does_not_resolve_is_refused_and_changes_nothing() {
    let mut doc = document();
    let before = serde_yaml::to_string(&doc).unwrap();
    let err = ProviderCatalogService::upsert_model(&mut doc, &ctx(), model_spec("m4", "ghost/api"))
        .unwrap_err();
    assert!(matches!(err, ConfigError::CatalogRefused(_)), "{err}");
    assert_eq!(serde_yaml::to_string(&doc).unwrap(), before);

    let err = ProviderCatalogService::upsert_provider(
        &mut doc,
        &ctx(),
        provider_spec("anthropic", "no-such-credential"),
    )
    .unwrap_err();
    assert!(matches!(err, ConfigError::CatalogRefused(_)), "{err}");
    assert_eq!(serde_yaml::to_string(&doc).unwrap(), before);
}

#[test]
fn removing_a_provider_still_deployed_is_refused() {
    let mut doc = document();
    let err =
        ProviderCatalogService::remove_provider(&mut doc, &ctx(), &ProviderId::new("anthropic"))
            .unwrap_err();
    assert!(matches!(err, ConfigError::CatalogRefused(_)), "{err}");
}

#[test]
fn removing_unknown_entities_names_them() {
    let mut doc = document();
    let err = ProviderCatalogService::remove_provider(&mut doc, &ctx(), &ProviderId::new("ghost"))
        .unwrap_err();
    assert_eq!(err.to_string(), "No provider named ghost");
    let err =
        ProviderCatalogService::remove_model(&mut doc, &ctx(), &ModelId::new("ghost")).unwrap_err();
    assert!(matches!(err, ConfigError::ModelNotFound { .. }));
    let err = ProviderCatalogService::remove_endpoint(
        &mut doc,
        &ctx(),
        &EndpointRef::new("anthropic", "ghost"),
    )
    .unwrap_err();
    assert!(matches!(err, ConfigError::EndpointNotFound { .. }));
}

#[test]
fn removing_a_model_removes_its_deployments() {
    let mut doc = document();
    ProviderCatalogService::remove_model(&mut doc, &ctx(), &ModelId::new("claude-internal"))
        .unwrap();
    assert!(!doc.models.models.contains_key("claude-internal"));
    assert!(!doc.deployments.deployments.contains_key("claude-internal"));
}

// Why: C-WP0 regression — re-declaring a provider or its endpoint must never
// drop its personal credential, user agent, forwarded/extra headers or betas.
#[test]
fn upserts_preserve_credential_user_agent_headers_and_betas() {
    let mut doc = opencode_go().document().clone();
    let mut spec = provider_spec("opencode-go", "opencode-go");
    spec.extra_headers
        .insert("x-extra".to_owned(), "1".to_owned());
    ProviderCatalogService::upsert_provider(&mut doc, &ctx(), spec).unwrap();
    ProviderCatalogService::upsert_endpoint(
        &mut doc,
        &ctx(),
        EndpointSpec {
            provider: ProviderId::new("opencode-go"),
            id: EndpointId::new("messages"),
            wire: WireProtocol::Anthropic,
            url: "https://moved.example/anthropic/v1".to_owned(),
        },
    )
    .unwrap();

    let provider = doc.providers.providers.get("opencode-go").unwrap();
    assert_eq!(provider.credential.as_str(), "opencode-go");
    assert_eq!(
        provider.user_agent.as_deref(),
        Some("systemprompt-gateway (opencode-go; {install})")
    );
    assert_eq!(
        provider.forward_headers,
        vec!["x-opencode-session".to_owned()]
    );
    assert_eq!(
        provider.extra_headers.get("x-extra").map(String::as_str),
        Some("1")
    );
    let messages = provider.endpoints.get("messages").unwrap();
    assert_eq!(messages.url, "https://moved.example/anthropic/v1");
    assert_eq!(
        messages.accepted_betas,
        Some(BetaSpec::List(BTreeSet::<AnthropicBeta>::new()))
    );
    assert!(
        doc.credentials
            .credentials
            .get("opencode-go")
            .unwrap()
            .kind
            .is_personal()
    );
}

#[test]
fn the_catalog_files_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let doc = opencode_go().document().clone();
    write_catalog(dir.path(), &doc).unwrap();
    for file in [
        "credentials.yaml",
        "providers.yaml",
        "models.yaml",
        "deployments.yaml",
    ] {
        assert!(dir.path().join(file).exists(), "{file}");
    }
    assert!(
        !dir.path().join("gateway.yaml").exists(),
        "gateway.yaml is never written"
    );
    let read = read_catalog(dir.path()).unwrap();
    assert_eq!(read.credentials, doc.credentials);
    assert_eq!(read.providers, doc.providers);
    assert_eq!(
        serde_yaml::to_string(&read.models).unwrap(),
        serde_yaml::to_string(&doc.models).unwrap()
    );
    assert_eq!(read.deployments, doc.deployments);
}
