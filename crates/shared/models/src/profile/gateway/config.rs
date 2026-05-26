use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::catalog::GatewayCatalog;
use super::error::{GatewayProfileError, GatewayResult};
use super::route::GatewayRoute;

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
