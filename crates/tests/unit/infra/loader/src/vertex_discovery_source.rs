//! Discovery as a provider capability: which source claims which provider, and
//! what a credential that no source accepts costs.

use std::time::Duration;

use async_trait::async_trait;
use systemprompt_loader::vertex_discovery::source::{
    CatalogListing, CatalogSource, DiscoveredModel, DiscoveryError, LaunchStage,
};
use systemprompt_loader::vertex_discovery::{Catalog, discover, discover_with};
use systemprompt_models::services::{
    DiscoveryReport, ProviderEntry, ProviderRegistry, VertexRateCard,
};
use systemprompt_security::credential::{
    AuthHeader, AuthScheme, CredentialKind, CredentialScope, ProviderCredential,
};

const VERTEX_PROVIDER: &str = r#"
name: vertex
wire: gemini
surface: gemini
endpoint: https://us-central1-aiplatform.googleapis.com/v1/projects/{project}/locations/us-central1/publishers/google
api_key_secret: vertex_key
"#;

fn registry() -> ProviderRegistry {
    ProviderRegistry {
        providers: vec![
            serde_yaml::from_str::<ProviderEntry>(VERTEX_PROVIDER)
                .expect("the fixture provider parses"),
        ],
    }
}

fn card() -> VertexRateCard {
    VertexRateCard::embedded().expect("the embedded rate card parses")
}

// Why: Vertex refuses API-key authentication as a class, so a provider keyed
// with one is not a discovery failure — there is simply nothing to ask. It
// must not reach the network, and it must not produce a report line.
#[tokio::test]
async fn discover_skips_providers_whose_credential_is_not_a_service_account() {
    let mut providers = registry();
    let lookup = |_: &str| Some("plain-api-key".to_owned());
    let report = discover(&mut providers, &lookup, Duration::from_millis(10)).await;

    assert!(report.failed_publishers.is_empty(), "{report:?}");
    assert!(report.discovered_priced.is_empty(), "{report:?}");
    assert!(report.priced_not_published.is_empty(), "{report:?}");
    assert!(providers.providers[0].models.is_empty());
}

#[tokio::test]
async fn a_provider_with_no_secret_at_all_is_left_alone() {
    let mut providers = registry();
    let lookup = |_: &str| None;
    let report = discover(&mut providers, &lookup, Duration::from_millis(10)).await;
    assert!(report.failed_publishers.is_empty(), "{report:?}");
    assert!(providers.providers[0].models.is_empty());
}

/// A source that claims the fixture provider on an API key and reports one
/// model the embedded rate card already prices.
struct FakeCatalog;

#[async_trait]
impl CatalogSource for FakeCatalog {
    fn name(&self) -> &'static str {
        "fake"
    }

    fn matches_provider(&self, provider: &ProviderEntry) -> bool {
        provider.name.as_str() == "vertex"
    }

    fn applies(&self, provider: &ProviderEntry, credential: &ProviderCredential) -> bool {
        self.matches_provider(provider) && credential.kind() == CredentialKind::ApiKey
    }

    async fn list(
        &self,
        _http: &reqwest::Client,
        auth: &AuthHeader,
        _provider: &ProviderEntry,
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
    let mut providers = registry();
    let card = card();
    let sources: Vec<Box<dyn CatalogSource>> = vec![Box::new(FakeCatalog)];
    let lookup = |_: &str| Some("plain-api-key".to_owned());
    let mut report = DiscoveryReport::default();

    discover_with(
        &mut providers,
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
        providers.providers[0]
            .find_model("vertex-gemini-3.5-flash")
            .is_some(),
        "the discovered model is appended to the registry"
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

    fn matches_provider(&self, provider: &ProviderEntry) -> bool {
        provider.name.as_str() == "vertex"
    }

    fn applies(&self, provider: &ProviderEntry, credential: &ProviderCredential) -> bool {
        self.matches_provider(provider) && credential.kind() == CredentialKind::ApiKey
    }

    async fn list(
        &self,
        _http: &reqwest::Client,
        _auth: &AuthHeader,
        _provider: &ProviderEntry,
        _scope: &CredentialScope,
    ) -> Result<CatalogListing, DiscoveryError> {
        if self.hang {
            std::future::pending().await
        } else {
            Err(DiscoveryError::Unusable(
                "fixture publisher refused catalog listing".to_owned(),
            ))
        }
    }
}

#[tokio::test]
async fn publisher_error_and_timeout_preserve_the_last_discovered_catalog() {
    let mut providers = registry();
    let card = card();
    let successful: Vec<Box<dyn CatalogSource>> = vec![Box::new(FakeCatalog)];
    let lookup = |_: &str| Some("plain-api-key".to_owned());
    let mut initial = DiscoveryReport::default();
    discover_with(
        &mut providers,
        &lookup,
        Duration::from_secs(1),
        Catalog {
            sources: &successful,
            card: &card,
        },
        &mut initial,
    )
    .await;
    let retained = providers.providers[0]
        .find_model("vertex-gemini-3.5-flash")
        .expect("successful discovery publishes priced model")
        .clone();

    for hang in [false, true] {
        let failing: Vec<Box<dyn CatalogSource>> = vec![Box::new(FailingCatalog { hang })];
        let mut report = DiscoveryReport::default();
        discover_with(
            &mut providers,
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
            assert!(failure.contains("refused catalog listing"), "{failure}");
        }
        let after = providers.providers[0]
            .find_model("vertex-gemini-3.5-flash")
            .expect("failed refresh preserves served catalog");
        assert_eq!(after.id, retained.id);
        assert_eq!(after.upstream_model, retained.upstream_model);
        assert_eq!(
            providers.providers[0]
                .models
                .iter()
                .filter(|model| model.id == retained.id)
                .count(),
            1,
            "failed refresh must not duplicate retained catalog entries"
        );
    }
}

struct MixedPolicyCatalog;

#[async_trait]
impl CatalogSource for MixedPolicyCatalog {
    fn name(&self) -> &'static str {
        "mixed-policy"
    }

    fn matches_provider(&self, provider: &ProviderEntry) -> bool {
        provider.name.as_str().starts_with("vertex")
    }

    fn applies(&self, provider: &ProviderEntry, credential: &ProviderCredential) -> bool {
        self.matches_provider(provider) && credential.kind() == CredentialKind::ApiKey
    }

    async fn list(
        &self,
        _http: &reqwest::Client,
        _auth: &AuthHeader,
        provider: &ProviderEntry,
        _scope: &CredentialScope,
    ) -> Result<CatalogListing, DiscoveryError> {
        assert_eq!(provider.name.as_str(), "vertex-maas");
        Ok(CatalogListing {
            models: vec![
                DiscoveredModel {
                    upstream: "qwen/qwen3-coder-480b-a35b-instruct-maas".to_owned(),
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

fn provider_with_model(name: &str, secret: &str, model: &str) -> ProviderEntry {
    serde_yaml::from_str(&format!(
        r#"
name: {name}
wire: openai-chat
surface: openai
endpoint: https://example.invalid/v1
api_key_secret: {secret}
models:
- id: {model}
  upstream_model: retained/{model}
  pricing:
    input_per_million: 1.0
    output_per_million: 2.0
"#
    ))
    .expect("provider fixture parses")
}

#[tokio::test]
async fn discovery_filters_the_mixed_listing_and_isolates_a_malformed_provider_credential() {
    let mut providers = ProviderRegistry {
        providers: vec![
            provider_with_model("vertex-maas", "good_vertex", "configured.vertex"),
            provider_with_model("vertex-bad", "bad_vertex", "configured.bad"),
            provider_with_model("other", "other_secret", "configured.other"),
        ],
    };
    let before_good =
        serde_json::to_value(&providers.providers[0].models).expect("configured models serialize");
    let before_bad = serde_json::to_value(&providers.providers[1].models)
        .expect("bad-provider models serialize");
    let before_other = serde_json::to_value(&providers.providers[2].models)
        .expect("unrelated-provider models serialize");
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
        &mut providers,
        &lookup,
        Duration::from_secs(1),
        Catalog {
            sources: &sources,
            card: &card,
        },
        &mut report,
    )
    .await;

    assert_eq!(report.discovered_priced, vec!["qwen.qwen3-coder-480b"]);
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
    let resulting_ids = providers.providers[0]
        .models
        .iter()
        .map(|model| model.id.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        resulting_ids,
        std::collections::BTreeSet::from(["configured.vertex", "qwen.qwen3-coder-480b"])
    );
    assert_eq!(
        serde_json::to_value(&providers.providers[0].models[0])
            .expect("retained configured model serializes"),
        before_good[0],
        "discovery appends a model without rewriting reviewed configuration"
    );
    assert!(
        providers.providers[0].find_model("zai.glm-5").is_none(),
        "preview model is withheld when the rate card does not opt in"
    );
    assert!(
        providers.providers[0]
            .find_model("qwen/unpriced-fixture-maas")
            .is_none()
    );
    assert!(
        providers.providers[0]
            .find_model("qwen/deployable-checkpoint")
            .is_none()
    );
    assert_eq!(
        serde_json::to_value(&providers.providers[1].models).expect("models serialize"),
        before_bad
    );
    assert_eq!(
        serde_json::to_value(&providers.providers[2].models).expect("models serialize"),
        before_other
    );
    assert!(
        report
            .priced_not_published
            .iter()
            .all(|id| id != "zai.glm-5"),
        "a listed preview is withheld, not misreported as absent: {report:?}"
    );
}
