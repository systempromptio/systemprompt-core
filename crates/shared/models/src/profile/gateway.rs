use crate::services::ai::ModelPricing;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
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
    "bearer".to_string()
}

fn default_inference_path_prefix() -> String {
    "/v1".to_string()
}

impl GatewayConfig {
    pub fn find_route(&self, model: &str) -> Option<&GatewayRoute> {
        self.routes.iter().find(|route| route.matches(model))
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
            if model.id.is_empty() {
                return Err(GatewayProfileError::ModelEmptyId);
            }
            if !self.providers.iter().any(|p| p.name == model.provider) {
                return Err(GatewayProfileError::UnknownProvider {
                    model: model.id.clone(),
                    provider: model.provider.clone(),
                });
            }
        }
        for provider in &self.providers {
            if provider.name.is_empty() {
                return Err(GatewayProfileError::ProviderEmptyName);
            }
            if provider.endpoint.is_empty() {
                return Err(GatewayProfileError::ProviderEmptyEndpoint {
                    name: provider.name.clone(),
                });
            }
        }
        Ok(())
    }

    pub fn find_provider(&self, name: &str) -> Option<&GatewayProvider> {
        self.providers.iter().find(|p| p.name == name)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GatewayProvider {
    pub name: String,
    pub endpoint: String,
    pub api_key_secret: String,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub extra_headers: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GatewayModel {
    pub id: String,
    pub provider: String,
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
    #[serde(default)]
    pub id: String,
    pub model_pattern: String,
    pub provider: String,
    pub endpoint: String,
    pub api_key_secret: String,
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
        if self.id.trim().is_empty() {
            self.id = synthesize_route_id(&self.model_pattern, &self.provider, &self.endpoint);
        }
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

/// Build a stable route id from `(model_pattern, provider, endpoint)`.
///
/// The id is `<slug>-<6 hex chars>` where the hex digest is the first 6
/// chars of `DefaultHasher` over the same triple. Mirrors the template
/// logic so ids stay identical across the seam.
#[must_use]
pub fn synthesize_route_id(model_pattern: &str, provider: &str, endpoint: &str) -> String {
    let mut hasher = DefaultHasher::new();
    model_pattern.hash(&mut hasher);
    provider.hash(&mut hasher);
    endpoint.hash(&mut hasher);
    let h = hasher.finish();
    let hash6: String = format!("{h:016x}").chars().take(6).collect();
    format!("{}-{}", slugify_pattern(model_pattern), hash6)
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
