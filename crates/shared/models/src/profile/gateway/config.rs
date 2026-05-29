//! Gateway configuration: on-disk spec and resolved runtime form.
//!
//! [`GatewayConfigSpec`] is the serde shape accepted under `gateway:` in a
//! profile; [`GatewayConfig`] is its post-resolution runtime projection with
//! any external catalog fully loaded. [`GatewayConfigSpec::resolve`] performs
//! the projection and catalog validation.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::catalog::GatewayCatalog;
use super::error::{GatewayProfileError, GatewayResult};
use super::route::GatewayRoute;

/// On-disk gateway configuration: the exact shape accepted under
/// `gateway:` in a profile YAML document.
///
/// Produced by serde deserialization; never holds a loaded catalog.
/// Project to the runtime [`GatewayConfig`] via [`Self::resolve`] once a
/// `profile_dir` is available.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GatewayConfigSpec {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub routes: Vec<GatewayRoute>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub catalog: Option<GatewayCatalogSource>,
    #[serde(default = "default_auth_scheme")]
    pub auth_scheme: String,
    #[serde(default = "default_inference_path_prefix")]
    pub inference_path_prefix: String,
}

/// Where a `gateway.catalog` block sources its providers and models.
///
/// Untagged: variants are disambiguated by their required keys.
/// `{ path: "..." }` reads an external catalog YAML; the inline form
/// carries `providers:` and `models:` directly.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(untagged, deny_unknown_fields)]
pub enum GatewayCatalogSource {
    Path { path: PathBuf },
    Inline(GatewayCatalog),
}

impl Default for GatewayConfigSpec {
    fn default() -> Self {
        Self {
            enabled: false,
            routes: Vec::new(),
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

impl GatewayConfigSpec {
    /// Project the on-disk spec to the runtime [`GatewayConfig`] by reading
    /// and validating any external catalog file referenced by
    /// [`GatewayCatalogSource::Path`].
    pub fn resolve(self, profile_dir: &Path) -> GatewayResult<GatewayConfig> {
        let Self {
            enabled,
            routes,
            catalog,
            auth_scheme,
            inference_path_prefix,
        } = self;

        let catalog = match catalog {
            None => None,
            Some(GatewayCatalogSource::Inline(c)) => {
                c.validate()?;
                Some(c)
            },
            Some(GatewayCatalogSource::Path { path: rel }) => {
                let absolute = if rel.is_absolute() {
                    rel
                } else {
                    profile_dir.join(rel)
                };
                let content = std::fs::read_to_string(&absolute).map_err(|source| {
                    GatewayProfileError::CatalogRead {
                        path: absolute.clone(),
                        source,
                    }
                })?;
                let parsed: GatewayCatalog = serde_yaml::from_str(&content).map_err(|source| {
                    GatewayProfileError::CatalogParse {
                        path: absolute.clone(),
                        source,
                    }
                })?;
                parsed
                    .validate()
                    .map_err(|source| GatewayProfileError::CatalogInvalid {
                        path: absolute.clone(),
                        source: Box::new(source),
                    })?;
                Some(parsed)
            },
        };

        Ok(GatewayConfig {
            enabled,
            routes,
            catalog,
            auth_scheme,
            inference_path_prefix,
        })
    }
}

/// Runtime gateway configuration: the post-resolution shape every
/// non-loader caller sees. The `catalog` field, when present, is fully
/// loaded — `Path` indirection has already been resolved.
///
/// Not `Deserialize`: the only legal construction paths are
/// [`GatewayConfigSpec::resolve`] for the production loader and direct
/// struct-literal construction in tests.
#[derive(Debug, Clone)]
pub struct GatewayConfig {
    pub enabled: bool,
    pub routes: Vec<GatewayRoute>,
    pub catalog: Option<GatewayCatalog>,
    pub auth_scheme: String,
    pub inference_path_prefix: String,
}

impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            routes: Vec::new(),
            catalog: None,
            auth_scheme: default_auth_scheme(),
            inference_path_prefix: default_inference_path_prefix(),
        }
    }
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

    /// Convert a runtime config back to the on-disk spec form, inlining the
    /// loaded catalog. Used when persisting a resolved profile back to YAML.
    #[must_use]
    pub fn to_spec(&self) -> GatewayConfigSpec {
        GatewayConfigSpec {
            enabled: self.enabled,
            routes: self.routes.clone(),
            catalog: self.catalog.clone().map(GatewayCatalogSource::Inline),
            auth_scheme: self.auth_scheme.clone(),
            inference_path_prefix: self.inference_path_prefix.clone(),
        }
    }
}
