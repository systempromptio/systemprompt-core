use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

    pub fn resolve_catalog(&mut self, profile_dir: &Path) -> GatewayResult<()> {
        let Some(rel) = self.catalog_path.as_ref() else {
            return Ok(());
        };
        let absolute = if rel.is_absolute() {
            rel.clone()
        } else {
            profile_dir.join(rel)
        };
        let content = std::fs::read_to_string(&absolute).map_err(|source| {
            GatewayProfileError::CatalogRead {
                path: absolute.clone(),
                source,
            }
        })?;
        let catalog: GatewayCatalog =
            serde_yaml::from_str(&content).map_err(|source| GatewayProfileError::CatalogParse {
                path: absolute.clone(),
                source,
            })?;
        catalog
            .validate()
            .map_err(|source| GatewayProfileError::CatalogInvalid {
                path: absolute.clone(),
                source: Box::new(source),
            })?;
        self.catalog_path = Some(absolute);
        self.catalog = Some(catalog);
        Ok(())
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayProvider {
    pub name: String,
    pub endpoint: String,
    pub api_key_secret: String,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub extra_headers: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayModel {
    pub id: String,
    pub provider: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream_model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayRoute {
    pub model_pattern: String,
    pub provider: String,
    pub endpoint: String,
    pub api_key_secret: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream_model: Option<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub extra_headers: HashMap<String, String>,
}

impl GatewayRoute {
    pub fn matches(&self, model: &str) -> bool {
        match_pattern(&self.model_pattern, model)
    }

    pub fn effective_upstream_model<'a>(&'a self, requested: &'a str) -> &'a str {
        self.upstream_model.as_deref().unwrap_or(requested)
    }
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
