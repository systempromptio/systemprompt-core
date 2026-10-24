//! Discovery as an endpoint capability: which source claims which catalog
//! endpoint, and what a credential that no source accepts costs.

use std::time::Duration;

use async_trait::async_trait;
use systemprompt_catalog_fixtures::{CatalogFixture, model, platform};
use systemprompt_loader::vertex_discovery::source::{
    CatalogListing, CatalogSource, DiscoveredModel, DiscoveryEndpoint, DiscoveryError, LaunchStage,
};
use systemprompt_loader::vertex_discovery::{Catalog, discover, discover_with};
use systemprompt_manifest::services::catalog::{
    CatalogDocument, EndpointConfig, ModelEntry, ProviderConfig,
};
use systemprompt_manifest::services::{DiscoveryReport, ModelPricing, VertexRateCard};
use systemprompt_models::providers::ClientFamily;
use systemprompt_security::credential::{
    AuthHeader, AuthScheme, CredentialKind, CredentialScope, ProviderCredential,
};
use systemprompt_wire::WireProtocol;

const VERTEX_GEMINI: &str = "https://us-central1-aiplatform.googleapis.com/v1/projects/{project}/locations/us-central1/publishers/google";

fn registry() -> CatalogDocument {
    CatalogFixture::new()
        .credential("vertex", platform("vertex_key"))
        .provider("vertex", ProviderConfig::new("vertex"))
        .endpoint(
            "vertex",
            "gemini",
            EndpointConfig::new(WireProtocol::Gemini, VERTEX_GEMINI),
        )
        .document()
        .clone()
}

fn card() -> VertexRateCard {
    VertexRateCard::embedded().expect("the embedded rate card parses")
}

// Why: Vertex refuses API-key authentication as a class, so an endpoint keyed
// with one is not a discovery failure — there is simply nothing to ask. It
// must not reach the network, and it must not produce a report line.
#[tokio::test]
async fn discover_skips_endpoints_whose_credential_is_not_a_service_account() {
    let mut document = registry();
    let lookup = |_: &str| Some("plain-api-key".to_owned());
    let report = discover(&mut document, &lookup, Duration::from_millis(10)).await;

    assert!(report.failed_publishers.is_empty(), "{report:?}");
    assert!(report.discovered_priced.is_empty(), "{report:?}");
    assert!(report.priced_not_published.is_empty(), "{report:?}");
    assert!(document.models.is_empty());
}

#[tokio::test]
async fn an_endpoint_with_no_secret_at_all_is_left_alone() {
    let mut document = registry();
    let lookup = |_: &str| None;
    let report = discover(&mut document, &lookup, Duration::from_millis(10)).await;
    assert!(report.failed_publishers.is_empty(), "{report:?}");
    assert!(document.models.is_empty());
}

struct FakeCatalog;

#[async_trait]
impl CatalogSource for FakeCatalog {
    fn name(&self) -> &'static str {
        "fake"
    }

    fn matches_endpoint(&self, endpoint: &DiscoveryEndpoint) -> bool {
        endpoint.endpoint.provider.as_str() == "vertex"
    }

    fn applies(&self, endpoint: &DiscoveryEndpoint, credential: &ProviderCredential) -> bool {
        self.matches_endpoint(endpoint) && credential.kind() == CredentialKind::ApiKey
    }

    async fn list(
        &self,
        _http: &reqwest::Client,
        auth: &AuthHeader,
        _endpoint: &DiscoveryEndpoint,
        scope: &CredentialScope,
    ) -> Result<CatalogListing, DiscoveryError> {
        assert_eq!(auth.scheme, AuthScheme::ApiKey);
        assert_eq!(scope, &CredentialScope::empty());
        Ok(CatalogListing {
            models: vec![DiscoveredModel {
                upstream: "google/gemini-3.5-flash".to_owned(),
                launch_stage: LaunchStage::GenerallyAvailable,
                serverless: true,
            }],
            failures: vec!["vertex/fake: one publisher did not list".to_owned()],
        })
    }
}

// Why: the point of the CatalogSource seam is that a new upstream is a new
// implementation and no change to the caller. This proves it with a source
// that shares none of Vertex's assumptions — an API key, no network, no
// Google — and still gets its models priced, merged and reported by exactly
// the same policy.
#[tokio::test]
async fn a_second_catalog_source_can_be_registered() {
    let mut document = registry();
    let card = card();
    let sources: Vec<Box<dyn CatalogSource>> = vec![Box::new(FakeCatalog)];
    let lookup = |_: &str| Some("plain-api-key".to_owned());
    let mut report = DiscoveryReport::default();

    discover_with(
        &mut document,
        &lookup,
        Duration::from_secs(5),
        Catalog {
            sources: &sources,
            card: &card,
        },
        &mut report,
    )
    .await;

    assert_eq!(report.discovered_priced, vec!["vertex-gemini-3.5-flash"]);
    assert_eq!(
        report.failed_publishers,
        vec!["vertex/fake: one publisher did not list"]
    );
    assert!(
        document.models.get("vertex-gemini-3.5-flash").is_some(),
        "the discovered model is appended to the catalog"
    );
    assert_eq!(
        document
            .deployments
            .get("vertex-gemini-3.5-flash")
            .map(|t| t[0].endpoint().to_string()),
        Some("vertex/gemini".to_owned()),
        "and deployed on the endpoint that listed it"
    );
    assert!(
        report
            .priced_not_published
            .iter()
            .all(|id| id != "vertex-gemini-3.5-flash"),
        "a model that listed is not also reported as unseen"
    );
}

struct FailingCatalog {
    hang: bool,
}

#[async_trait]
impl CatalogSource for FailingCatalog {
    fn name(&self) -> &'static str {
        "failing-fixture"
    }

    fn matches_endpoint(&self, endpoint: &DiscoveryEndpoint) -> bool {
        endpoint.endpoint.provider.as_str() == "vertex"
    }

    fn applies(&self, endpoint: &DiscoveryEndpoint, credential: &ProviderCredential) -> bool {
        self.matches_endpoint(endpoint) && credential.kind() == CredentialKind::ApiKey
    }

    async fn list(
        &self,
        _http: &reqwest::Client,
        _auth: &AuthHeader,
        _endpoint: &DiscoveryEndpoint,
        _scope: &CredentialScope,
    ) -> Result<CatalogListing, DiscoveryError> {
        if self.hang {
            std::future::pending().await
        } else {
            Err(DiscoveryError::BearerRequired)
        }
    }
}

#[tokio::test]
async fn publisher_error_and_timeout_preserve_the_last_discovered_catalog() {
    let mut document = registry();
    let card = card();
    let successful: Vec<Box<dyn CatalogSource>> = vec![Box::new(FakeCatalog)];
    let lookup = |_: &str| Some("plain-api-key".to_owned());
    let mut initial = DiscoveryReport::default();
    discover_with(
        &mut document,
        &lookup,
        Duration::from_secs(1),
        Catalog {
            sources: &successful,
            card: &card,
        },
        &mut initial,
    )
    .await;
    let retained = document
        .models
        .get("vertex-gemini-3.5-flash")
        .expect("successful discovery publishes priced model")
        .clone();

    for hang in [false, true] {
        let failing: Vec<Box<dyn CatalogSource>> = vec![Box::new(FailingCatalog { hang })];
        let mut report = DiscoveryReport::default();
        discover_with(
            &mut document,
            &lookup,
            Duration::from_millis(1),
            Catalog {
                sources: &failing,
                card: &card,
            },
            &mut report,
        )
        .await;

        assert_eq!(report.failed_publishers.len(), 1);
        let failure = &report.failed_publishers[0];
        if hang {
            assert!(failure.contains("timed out after 0s"), "{failure}");
        } else {
            assert!(failure.contains("requires a bearer token"), "{failure}");
        }
        let after = document
            .models
            .get("vertex-gemini-3.5-flash")
            .expect("failed refresh preserves served catalog");
        assert_eq!(after.upstream_model, retained.upstream_model);
        assert_eq!(document.models.models.len(), 1);
    }
}

struct MixedPolicyCatalog;

#[async_trait]
impl CatalogSource for MixedPolicyCatalog {
    fn name(&self) -> &'static str {
        "mixed-policy"
    }

    fn matches_endpoint(&self, endpoint: &DiscoveryEndpoint) -> bool {
        endpoint.endpoint.provider.as_str().starts_with("vertex")
    }

    fn applies(&self, endpoint: &DiscoveryEndpoint, credential: &ProviderCredential) -> bool {
        self.matches_endpoint(endpoint) && credential.kind() == CredentialKind::ApiKey
    }

    async fn list(
        &self,
        _http: &reqwest::Client,
        _auth: &AuthHeader,
        endpoint: &DiscoveryEndpoint,
        _scope: &CredentialScope,
    ) -> Result<CatalogListing, DiscoveryError> {
        assert_eq!(endpoint.endpoint.provider.as_str(), "vertex-maas");
        Ok(CatalogListing {
            models: vec![
                DiscoveredModel {
                    upstream: "openai/gpt-oss-120b-maas".to_owned(),
                    launch_stage: LaunchStage::GenerallyAvailable,
                    serverless: true,
                },
                DiscoveredModel {
                    upstream: "zai-org/glm-5-maas".to_owned(),
                    launch_stage: LaunchStage::Preview,
                    serverless: true,
                },
                DiscoveredModel {
                    upstream: "qwen/unpriced-fixture-maas".to_owned(),
                    launch_stage: LaunchStage::GenerallyAvailable,
                    serverless: true,
                },
                DiscoveredModel {
                    upstream: "qwen/deployable-checkpoint".to_owned(),
                    launch_stage: LaunchStage::GenerallyAvailable,
                    serverless: false,
                },
            ],
            failures: Vec::new(),
        })
    }
}

fn configured(model_id: &str) -> ModelEntry {
    ModelEntry {
        upstream_model: Some(format!("retained/{model_id}")),
        ..model(
            &[ClientFamily::OpenAi],
            0,
            ModelPricing {
                input_per_million: 1.0,
                output_per_million: 2.0,
                cache_read_per_million: Some(0.0),
                ..ModelPricing::default()
            },
        )
    }
}

fn with_provider(
    fixture: CatalogFixture,
    name: &str,
    secret: &str,
    model_id: &str,
) -> CatalogFixture {
    fixture
        .credential(name, platform(secret))
        .provider(name, ProviderConfig::new(name))
        .endpoint(
            name,
            "chat",
            EndpointConfig::new(WireProtocol::OpenAiChat, "https://example.invalid/v1"),
        )
        .model(model_id, configured(model_id))
        .deploy(model_id, [format!("{name}/chat").as_str()])
}

#[tokio::test]
async fn discovery_filters_the_mixed_listing_and_isolates_a_malformed_credential() {
    let fixture = with_provider(
        CatalogFixture::new(),
        "vertex-maas",
        "good_vertex",
        "configured.vertex",
    );
    let fixture = with_provider(fixture, "vertex-bad", "bad_vertex", "configured.bad");
    let fixture = with_provider(fixture, "other", "other_secret", "configured.other");
    let mut document = fixture.document().clone();
    let before = serde_json::to_value(&document.models).expect("models serialize");
    let mut card = card();
    for entry in &mut card.entries {
        entry.allow_preview = false;
    }
    let sources: Vec<Box<dyn CatalogSource>> = vec![Box::new(MixedPolicyCatalog)];
    let lookup = |name: &str| match name {
        "good_vertex" => Some("fixture-api-key".to_owned()),
        "bad_vertex" => Some(r#"{"type":"service_account"}"#.to_owned()),
        "other_secret" => panic!("an unrelated provider secret must not be read"),
        _ => None,
    };
    let mut report = DiscoveryReport::default();

    discover_with(
        &mut document,
        &lookup,
        Duration::from_secs(1),
        Catalog {
            sources: &sources,
            card: &card,
        },
        &mut report,
    )
    .await;

    assert_eq!(report.discovered_priced, vec!["openai.gpt-oss-120b"]);
    assert_eq!(
        report.discovered_unpriced,
        vec!["qwen/unpriced-fixture-maas"]
    );
    assert_eq!(report.failed_publishers.len(), 1, "{report:?}");
    assert!(
        report.failed_publishers[0].starts_with("vertex-bad: service-account key is malformed:"),
        "{:?}",
        report.failed_publishers
    );
    let ids: std::collections::BTreeSet<&str> =
        document.models.iter().map(|(id, _)| id.as_str()).collect();
    assert_eq!(
        ids,
        std::collections::BTreeSet::from([
            "configured.bad",
            "configured.other",
            "configured.vertex",
            "openai.gpt-oss-120b"
        ])
    );
    let after = serde_json::to_value(&document.models).expect("models serialize");
    for id in ["configured.vertex", "configured.bad", "configured.other"] {
        assert_eq!(
            after[id], before[id],
            "discovery appends a model without rewriting reviewed configuration"
        );
    }
    assert!(
        document.models.get("zai.glm-5").is_none(),
        "preview model is withheld when the rate card does not opt in"
    );
    assert!(
        report
            .priced_not_published
            .iter()
            .all(|id| id != "zai.glm-5"),
        "a listed preview is withheld, not misreported as absent: {report:?}"
    );
}
