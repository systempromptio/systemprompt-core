//! Runtime projection of the gateway configuration.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::ModelId;

use crate::services::gateway::config::{
    BridgeReleasesSpec, GatewayConfigSpec, QuotaFaultMode, default_auth_scheme,
    default_inference_path_prefix,
};
use crate::services::gateway::override_rule::SystemPromptRule;
use crate::services::gateway::route::GatewayRoute;

/// Runtime gateway configuration: the post-resolution shape every non-loader
/// caller sees.
///
/// Not `Deserialize`: the only legal construction paths are
/// [`GatewayConfigSpec::resolve`] for the production loader and direct
/// struct-literal construction in tests.
#[derive(Debug, Clone)]
pub struct GatewayConfig {
    pub enabled: bool,
    pub routes: Vec<GatewayRoute>,
    pub default_model: Option<ModelId>,
    pub quota_fault_mode: QuotaFaultMode,
    pub require_scopes: Vec<systemprompt_identifiers::ScopeDimension>,
    pub automatic_prompt_caching: bool,
    pub auth_scheme: String,
    pub inference_path_prefix: String,
    pub system_prompt_overrides: Vec<SystemPromptRule>,
    pub bridge_releases: Option<BridgeReleasesSpec>,
}

impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            routes: Vec::new(),
            default_model: None,
            quota_fault_mode: QuotaFaultMode::default(),
            require_scopes: Vec::new(),
            automatic_prompt_caching: true,
            auth_scheme: default_auth_scheme(),
            inference_path_prefix: default_inference_path_prefix(),
            system_prompt_overrides: Vec::new(),
            bridge_releases: None,
        }
    }
}

impl GatewayConfig {
    #[must_use]
    pub fn find_route(&self, id: &str) -> Option<&GatewayRoute> {
        self.routes.iter().find(|route| route.id.as_str() == id)
    }

    #[must_use]
    pub fn to_spec(&self) -> GatewayConfigSpec {
        GatewayConfigSpec {
            enabled: self.enabled,
            routes: self.routes.clone(),
            default_model: self.default_model.clone(),
            quota_fault_mode: self.quota_fault_mode,
            require_scopes: self.require_scopes.clone(),
            automatic_prompt_caching: self.automatic_prompt_caching,
            auth_scheme: self.auth_scheme.clone(),
            inference_path_prefix: self.inference_path_prefix.clone(),
            system_prompt_overrides: self.system_prompt_overrides.clone(),
            bridge_releases: self.bridge_releases.clone(),
        }
    }
}
