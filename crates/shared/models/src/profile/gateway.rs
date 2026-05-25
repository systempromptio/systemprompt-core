use crate::services::ai::ModelPricing;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use systemprompt_identifiers::{ModelId, ProviderId, RouteId, SecretName};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GatewayProfileError {
    #[error("Failed to read gateway catalog {path}: {source}")]
    CatalogRead {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to parse gateway catalog {path}: {source}")]
    CatalogParse {
        path: PathBuf,
        #[source]
        source: serde_yaml::Error,
    },

    #[error("Invalid gateway catalog {path}: {source}")]
    CatalogInvalid {
        path: PathBuf,
        #[source]
        source: Box<Self>,
    },

    #[error("gateway catalog model has empty id")]
    ModelEmptyId,

    #[error("gateway catalog model '{model}' references unknown provider '{provider}'")]
    UnknownProvider { model: String, provider: String },

    #[error("gateway catalog provider has empty name")]
    ProviderEmptyName,

    #[error("gateway catalog provider '{name}' has empty endpoint")]
    ProviderEmptyEndpoint { name: String },

    #[error("gateway {label} endpoint '{endpoint}' is not permitted: {reason}")]
    BlockedEndpoint {
        label: String,
        endpoint: String,
        reason: String,
    },

    #[error(
        "gateway route '{route}' provider '{provider}' is not declared in the catalog providers"
    )]
    RouteProviderNotInCatalog { route: String, provider: String },

    #[error("gateway catalog model id or alias '{id}' is declared more than once")]
    DuplicateModelId { id: String },

    #[error("gateway route id '{id}' is declared more than once")]
    DuplicateRouteId { id: String },

    #[error("gateway catalog model '{model}' has no route whose pattern matches its id")]
    UnreachableModel { model: String },
}

/// Reject gateway upstream endpoints that point at the local host or private
/// network ranges; an operator-configured endpoint pointing at
/// `169.254.169.254` or an internal service would otherwise turn the inference
/// proxy into an SSRF primitive. Delegates to the shared outbound-URL guard so
/// gateway, webhook, and authz destinations enforce one policy.
fn validate_endpoint(label: &str, endpoint: &str) -> GatewayResult<()> {
    crate::net::validate_outbound_url(endpoint)
        .map(|_| ())
        .map_err(|e| GatewayProfileError::BlockedEndpoint {
            label: label.to_owned(),
            endpoint: endpoint.to_owned(),
            reason: e.to_string(),
        })
}

pub type GatewayResult<T> = Result<T, GatewayProfileError>;

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GatewayConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub routes: Vec<GatewayRoute>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub catalog_path: Option<PathBuf>,
    #[serde(default, skip)]
    pub catalog: Option<GatewayCatalog>,
    #[serde(default = "default_auth_scheme")]
    pub auth_scheme: String,
    #[serde(default = "default_inference_path_prefix")]
    pub inference_path_prefix: String,
}

impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            routes: Vec::new(),
            catalog_path: None,
            catalog: None,
            auth_scheme: default_auth_scheme(),
            inference_path_prefix: default_inference_path_prefix(),
        }
    }
}

fn default_auth_scheme() -> String {
    "bearer".to_owned()
}

fn default_inference_path_prefix() -> String {
    "/v1".to_owned()
}

fn default_route_id() -> RouteId {
    RouteId::new("")
}

impl GatewayConfig {
    pub fn find_route(&self, model: &str) -> Option<&GatewayRoute> {
        self.routes.iter().find(|route| route.matches(model))
    }

    #[must_use]
    pub fn is_model_exposed(&self, model: &str) -> bool {
        self.catalog
            .as_ref()
            .is_none_or(|c| c.contains_model(model))
    }

    pub fn validate(&self) -> GatewayResult<()> {
        let mut route_ids: std::collections::HashSet<&str> =
            std::collections::HashSet::with_capacity(self.routes.len());
        for route in &self.routes {
            if !route_ids.insert(route.id.as_str()) {
                return Err(GatewayProfileError::DuplicateRouteId {
                    id: route.id.as_str().to_owned(),
                });
            }
        }
        let Some(catalog) = self.catalog.as_ref() else {
            return Ok(());
        };
        catalog.validate()?;
        for route in &self.routes {
            if catalog.find_provider(route.provider.as_str()).is_none() {
                return Err(GatewayProfileError::RouteProviderNotInCatalog {
                    route: route.model_pattern.clone(),
                    provider: route.provider.as_str().to_owned(),
                });
            }
        }
        let mut seen = std::collections::HashSet::with_capacity(catalog.models.len());
        for model in &catalog.models {
            if !seen.insert(model.id.as_str()) {
                return Err(GatewayProfileError::DuplicateModelId {
                    id: model.id.as_str().to_owned(),
                });
            }
            for alias in &model.aliases {
                if !seen.insert(alias.as_str()) {
                    return Err(GatewayProfileError::DuplicateModelId {
                        id: alias.as_str().to_owned(),
                    });
                }
            }
            if !self.routes.iter().any(|r| r.matches(model.id.as_str())) {
                return Err(GatewayProfileError::UnreachableModel {
                    model: model.id.as_str().to_owned(),
                });
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GatewayCatalog {
    #[serde(default)]
    pub providers: Vec<GatewayProvider>,
    #[serde(default)]
    pub models: Vec<GatewayModel>,
}

impl GatewayCatalog {
    pub fn validate(&self) -> GatewayResult<()> {
        for model in &self.models {
            if model.id.as_str().is_empty() {
                return Err(GatewayProfileError::ModelEmptyId);
            }
            if !self.providers.iter().any(|p| p.name == model.provider) {
                return Err(GatewayProfileError::UnknownProvider {
                    model: model.id.as_str().to_owned(),
                    provider: model.provider.as_str().to_owned(),
                });
            }
        }
        for provider in &self.providers {
            if provider.name.as_str().is_empty() {
                return Err(GatewayProfileError::ProviderEmptyName);
            }
            if provider.endpoint.is_empty() {
                return Err(GatewayProfileError::ProviderEmptyEndpoint {
                    name: provider.name.as_str().to_owned(),
                });
            }
            validate_endpoint(
                &format!("provider '{}'", provider.name.as_str()),
                &provider.endpoint,
            )?;
        }
        Ok(())
    }

    pub fn find_provider(&self, name: &str) -> Option<&GatewayProvider> {
        self.providers.iter().find(|p| p.name.as_str() == name)
    }

    #[must_use]
    pub fn contains_model(&self, requested: &str) -> bool {
        self.models.iter().any(|m| {
            m.id.as_str() == requested || m.aliases.iter().any(|a| a.as_str() == requested)
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GatewayProvider {
    pub name: ProviderId,
    pub endpoint: String,
    pub api_key_secret: SecretName,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub extra_headers: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GatewayModel {
    pub id: ModelId,
    pub provider: ProviderId,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<ModelId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream_model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pricing: Option<ModelPricing>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GatewayRoute {
    #[serde(default = "default_route_id")]
    pub id: RouteId,
    pub model_pattern: String,
    pub provider: ProviderId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream_model: Option<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub extra_headers: HashMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pricing: Option<ModelPricing>,
}

impl GatewayRoute {
    pub fn matches(&self, model: &str) -> bool {
        match_pattern(&self.model_pattern, model)
    }

    pub fn effective_upstream_model<'a>(&'a self, requested: &'a str) -> &'a str {
        self.upstream_model.as_deref().unwrap_or(requested)
    }

    pub fn ensure_id(&mut self) {
        if self.id.as_str().trim().is_empty() {
            self.id = synthesize_route_id(&self.model_pattern, self.provider.as_str());
        }
    }

    pub fn resolve<'a>(&self, providers: &'a [GatewayProvider]) -> Option<&'a GatewayProvider> {
        providers.iter().find(|p| p.name == self.provider)
    }
}

/// Slugify a model pattern for use in a stable id.
///
/// Mirrors the template's historical implementation in
/// `extensions/web/admin/.../gateway.rs`: `*` becomes `star`,
/// non-alphanumeric runs collapse to a single `-`, leading/trailing `-`
/// are trimmed, and an empty result becomes `route`.
#[must_use]
pub fn slugify_pattern(pattern: &str) -> String {
    let mut out = String::with_capacity(pattern.len());
    let mut last_dash = false;
    for ch in pattern.chars() {
        if ch == '*' {
            out.push_str("star");
            last_dash = false;
        } else if ch.is_ascii_alphanumeric() {
            for lc in ch.to_lowercase() {
                out.push(lc);
            }
            last_dash = false;
        } else if !last_dash && !out.is_empty() {
            out.push('-');
            last_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    while out.starts_with('-') {
        out.remove(0);
    }
    if out.is_empty() {
        out.push_str("route");
    }
    out
}

// Format: <slug>-<6 hex chars> where the hex digest is the first 6 chars of
// DefaultHasher over (model_pattern, provider). The collision check in
// GatewayConfig::validate() guards against the vanishingly unlikely case of
// two operator-authored patterns colliding on the 6-hex tail.
#[must_use]
pub fn synthesize_route_id(model_pattern: &str, provider: &str) -> RouteId {
    let mut hasher = DefaultHasher::new();
    model_pattern.hash(&mut hasher);
    provider.hash(&mut hasher);
    let h = hasher.finish();
    let hash6: String = format!("{h:016x}").chars().take(6).collect();
    RouteId::new(format!("{}-{}", slugify_pattern(model_pattern), hash6))
}

fn match_pattern(pattern: &str, model: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if let Some(prefix) = pattern.strip_suffix('*') {
        return model.starts_with(prefix);
    }
    if let Some(suffix) = pattern.strip_prefix('*') {
        return model.ends_with(suffix);
    }
    pattern == model
}
