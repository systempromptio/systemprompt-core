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
                upstream: "google/gemini-2.5-pro".to_owned(),
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

    assert_eq!(report.discovered_priced, vec!["vertex-gemini-2.5-pro"]);
    assert_eq!(
        report.failed_publishers,
        vec!["vertex/fake: one publisher did not list"]
    );
    assert!(
        providers.providers[0]
            .find_model("vertex-gemini-2.5-pro")
            .is_some(),
        "the discovered model is appended to the registry"
    );
    assert!(
        report
            .priced_not_published
            .iter()
            .all(|id| id != "vertex-gemini-2.5-pro"),
        "a model that listed is not also reported as unseen"
    );
}
