//! A builder for [`ResolvedCatalog`] and the canned catalogs tests share.
//!
//! ```ignore
//! let catalog = CatalogFixture::new()
//!     .credential("anthropic", platform("anthropic"))
//!     .provider("anthropic", ProviderConfig::new("anthropic"))
//!     .endpoint("anthropic", "api", EndpointConfig::new(WireProtocol::Anthropic, URL))
//!     .model("claude-x", claude_model(200_000))
//!     .deploy("claude-x", ["anthropic/api"])
//!     .default_model("claude-x")
//!     .build();
//! ```
//!
//! `build` panics with the catalog error, so a fixture that does not resolve
//! fails the test that uses it; `try_build` returns the error for tests of
//! validation itself.

mod canned;

use std::collections::BTreeMap;

use systemprompt_identifiers::{CredentialId, EndpointId, ModelId, ProviderId, SecretName};
use systemprompt_manifest::services::catalog::{
    CATALOG_FILES, CatalogContext, CatalogDocument, CatalogDocumentError, CatalogInput,
    CatalogResult, CredentialEntry, DefaultCatalog, DeploymentTarget, EndpointConfig, EndpointRef,
    ModelEntry, ProviderConfig, ResolvedCatalog,
};
use systemprompt_manifest::services::{GatewayConfigSpec, GatewayRoute, ModelPricing};
use systemprompt_models::providers::ClientFamily;
use systemprompt_wire::ModelLimits;

pub use canned::{
    OPENCODE_GO_CHAT_URL, OPENCODE_GO_MESSAGES_URL, one_platform_provider, opencode_go,
    vertex_regions,
};

#[must_use]
pub fn platform(secret: &str) -> CredentialEntry {
    CredentialEntry::platform(SecretName::new(secret))
}

#[must_use]
pub fn personal(label: &str) -> CredentialEntry {
    CredentialEntry::personal(Some(label.to_owned()))
}

#[must_use]
pub const fn priced(input: f64, output: f64, cache_read: f64) -> ModelPricing {
    ModelPricing {
        input_per_million: input,
        output_per_million: output,
        cache_read_per_million: Some(cache_read),
        cache_write_per_million: None,
        per_image_cents: None,
    }
}

#[must_use]
pub fn model(clients: &[ClientFamily], context_window: u32, pricing: ModelPricing) -> ModelEntry {
    ModelEntry {
        limits: ModelLimits {
            context_window,
            max_output_tokens: 8_192,
            max_thinking_budget: None,
        },
        pricing,
        ..ModelEntry::new(clients.to_vec())
    }
}

#[must_use]
pub fn claude_model(context_window: u32) -> ModelEntry {
    model(
        &[ClientFamily::Claude],
        context_window,
        priced(3.0, 15.0, 0.3),
    )
}

#[derive(Debug, Clone, Default)]
pub struct CatalogFixture {
    document: CatalogDocument,
    context: CatalogContext,
    default_image_model: Option<ModelId>,
}

impl CatalogFixture {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn from_document(document: CatalogDocument) -> Self {
        Self {
            document,
            ..Self::default()
        }
    }

    /// Core's embedded default catalog (`DefaultCatalog`) as a fixture to edit.
    #[must_use]
    pub fn embedded_default() -> Self {
        Self::from_document(
            DefaultCatalog::document()
                .unwrap_or_else(|e| panic!("embedded default catalog parses: {e}")),
        )
    }

    #[must_use]
    pub fn credential(mut self, id: &str, entry: CredentialEntry) -> Self {
        self.document
            .credentials
            .credentials
            .insert(CredentialId::new(id), entry);
        self
    }

    #[must_use]
    pub fn provider(mut self, id: &str, provider: ProviderConfig) -> Self {
        self.document
            .providers
            .providers
            .insert(ProviderId::new(id), provider);
        self
    }

    #[must_use]
    pub fn with_provider(mut self, id: &str, edit: impl FnOnce(&mut ProviderConfig)) -> Self {
        let provider = self
            .document
            .providers
            .providers
            .get_mut(id)
            .unwrap_or_else(|| panic!("fixture provider '{id}' is not declared"));
        edit(provider);
        self
    }

    #[must_use]
    pub fn endpoint(self, provider: &str, id: &str, endpoint: EndpointConfig) -> Self {
        self.with_provider(provider, |p| {
            p.endpoints.insert(EndpointId::new(id), endpoint);
        })
    }

    #[must_use]
    pub fn model(mut self, id: &str, entry: ModelEntry) -> Self {
        self.document.models.models.insert(ModelId::new(id), entry);
        self
    }

    #[must_use]
    pub fn deploy<T: Into<FixtureTarget>>(
        mut self,
        model: &str,
        targets: impl IntoIterator<Item = T>,
    ) -> Self {
        let targets = targets.into_iter().map(|t| t.into().0).collect();
        self.document
            .deployments
            .deployments
            .insert(ModelId::new(model), targets);
        self
    }

    #[must_use]
    pub fn route(mut self, route: GatewayRoute) -> Self {
        self.gateway_mut().routes.push(route);
        self
    }

    #[must_use]
    pub fn default_model(mut self, model: &str) -> Self {
        self.gateway_mut().default_model = Some(ModelId::new(model));
        self
    }

    #[must_use]
    pub fn without_default_model(mut self) -> Self {
        self.gateway_mut().default_model = None;
        self
    }

    #[must_use]
    pub fn gateway_enabled(mut self, enabled: bool) -> Self {
        self.gateway_mut().enabled = enabled;
        self
    }

    #[must_use]
    pub fn install(mut self, install: &str) -> Self {
        self.context = CatalogContext::for_install(install);
        self
    }

    fn gateway_mut(&mut self) -> &mut GatewayConfigSpec {
        self.document
            .gateway
            .get_or_insert_with(GatewayConfigSpec::default)
    }

    #[must_use]
    pub fn document(&self) -> &CatalogDocument {
        &self.document
    }

    #[must_use]
    pub fn default_image_model(mut self, model: &str) -> Self {
        self.default_image_model = Some(ModelId::new(model));
        self
    }

    pub fn try_build(&self) -> CatalogResult<ResolvedCatalog> {
        let gateway = self
            .document
            .gateway
            .clone()
            .map(GatewayConfigSpec::resolve);
        ResolvedCatalog::build(
            CatalogInput {
                credentials: &self.document.credentials,
                providers: &self.document.providers,
                models: &self.document.models,
                deployments: &self.document.deployments,
                gateway: gateway.as_ref(),
                default_image_model: self.default_image_model.as_ref(),
            },
            &self.context,
        )
    }

    #[must_use]
    pub fn build(&self) -> ResolvedCatalog {
        self.try_build()
            .unwrap_or_else(|e| panic!("catalog fixture does not resolve: {e}"))
    }
}

/// A deployment target written as `"provider/endpoint"` or given in full.
pub struct FixtureTarget(DeploymentTarget);

impl From<&str> for FixtureTarget {
    fn from(raw: &str) -> Self {
        Self(DeploymentTarget::Endpoint(
            EndpointRef::parse(raw).unwrap_or_else(|e| panic!("{e}")),
        ))
    }
}

impl From<DeploymentTarget> for FixtureTarget {
    fn from(target: DeploymentTarget) -> Self {
        Self(target)
    }
}

#[must_use]
pub fn endpoints(entries: &[(&str, EndpointConfig)]) -> BTreeMap<EndpointId, EndpointConfig> {
    entries
        .iter()
        .map(|(id, endpoint)| (EndpointId::new(*id), endpoint.clone()))
        .collect()
}

/// Reads the catalog files of a `services/ai` directory, in
/// [`CATALOG_FILES`] order, into one merged document.
pub fn read_ai_dir(dir: &std::path::Path) -> Result<CatalogDocument, String> {
    let mut merged = CatalogDocument::default();
    for file in CATALOG_FILES {
        let path = dir.join(file);
        if !path.exists() {
            continue;
        }
        let origin = path.display().to_string();
        let text = std::fs::read_to_string(&path).map_err(|e| format!("{origin}: {e}"))?;
        let document = CatalogDocument::from_yaml_str(&origin, &text).map_err(render)?;
        merged.merge(document).map_err(render)?;
    }
    Ok(merged)
}

fn render(error: CatalogDocumentError) -> String {
    error.to_string()
}

/// One resolved hop for a test that needs an outbound target: provider
/// `provider` with a platform credential of the same name, one endpoint `api`
/// on `wire` at `url`, serving model `model` as `upstream_model`.
#[must_use]
pub fn hop(
    provider: &str,
    wire: systemprompt_wire::WireProtocol,
    url: &str,
    model_id: &str,
    upstream_model: &str,
) -> systemprompt_manifest::services::catalog::ResolvedHop {
    let entry = ModelEntry {
        upstream_model: Some(upstream_model.to_owned()),
        ..model(&[ClientFamily::Claude], 200_000, priced(1.0, 2.0, 0.1))
    };
    let catalog = CatalogFixture::new()
        .credential(provider, platform(provider))
        .provider(provider, ProviderConfig::new(provider))
        .endpoint(provider, "api", EndpointConfig::new(wire, url))
        .model(model_id, entry)
        .deploy(model_id, [format!("{provider}/api").as_str()])
        .build();
    catalog.deployments(model_id)[0].clone()
}
