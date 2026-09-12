//! Gateway configuration: on-disk spec and resolved runtime form.
//!
//! [`GatewayConfigSpec`] is the serde shape accepted under `gateway:` in the
//! services tree; [`GatewayConfig`] is its runtime projection. Routes carry no
//! embedded provider catalog — every route resolves its provider against
//! `services.providers` (the merged `providers:` list of the services tree)
//! (`ProviderRegistry`) at use time.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod runtime;
mod validate;

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::ProviderId;

use crate::services::gateway::override_rule::SystemPromptRule;
use crate::services::gateway::route::GatewayRoute;

pub use runtime::GatewayConfig;

pub(crate) const DEFAULT_ROUTE_PATTERN: &str = "*";

/// What the gateway does when it cannot evaluate a quota or policy.
///
/// The switch lives in file config rather than in `GatewayPolicySpec` because
/// one of the faults it governs is the failure to read that policy row.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum QuotaFaultMode {
    #[default]
    Open,
    Closed,
}

impl QuotaFaultMode {
    #[must_use]
    pub const fn is_closed(self) -> bool {
        matches!(self, Self::Closed)
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Closed => "closed",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GatewayConfigSpec {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub routes: Vec<GatewayRoute>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_provider: Option<ProviderId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_model: Option<String>,
    #[serde(default)]
    pub allow_unlisted_models: bool,
    #[serde(default)]
    pub quota_fault_mode: QuotaFaultMode,
    #[serde(default = "default_auth_scheme")]
    pub auth_scheme: String,
    #[serde(default = "default_inference_path_prefix")]
    pub inference_path_prefix: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub system_prompt_overrides: Vec<SystemPromptRule>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bridge_releases: Option<BridgeReleasesSpec>,
}

/// Release feed for the desktop bridge self-updater.
///
/// The bridge cannot reach these assets itself — the repository is private —
/// so the gateway resolves and proxies them. Keeping the resolution here is
/// also what makes staged rollouts a config change rather than a client
/// release.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BridgeReleasesSpec {
    pub repo: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_secret: Option<String>,
    #[serde(default = "default_tag_prefix")]
    pub tag_prefix: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pinned_version: Option<String>,
    #[serde(default)]
    pub assets: std::collections::BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_base: Option<String>,
}

impl BridgeReleasesSpec {
    #[must_use]
    pub fn api_base(&self) -> &str {
        self.api_base.as_deref().unwrap_or("https://api.github.com")
    }
}

fn default_tag_prefix() -> String {
    "bridge-v".to_owned()
}

impl Default for GatewayConfigSpec {
    fn default() -> Self {
        Self {
            enabled: false,
            routes: Vec::new(),
            default_provider: None,
            default_model: None,
            allow_unlisted_models: false,
            quota_fault_mode: QuotaFaultMode::default(),
            auth_scheme: default_auth_scheme(),
            inference_path_prefix: default_inference_path_prefix(),
            system_prompt_overrides: Vec::new(),
            bridge_releases: None,
        }
    }
}

pub(crate) fn default_auth_scheme() -> String {
    "bearer".to_owned()
}

pub(crate) fn default_inference_path_prefix() -> String {
    "/v1".to_owned()
}

impl GatewayConfigSpec {
    #[must_use]
    pub fn resolve(self) -> GatewayConfig {
        let Self {
            enabled,
            routes,
            default_provider,
            default_model,
            allow_unlisted_models,
            quota_fault_mode,
            auth_scheme,
            inference_path_prefix,
            system_prompt_overrides,
            bridge_releases,
        } = self;

        GatewayConfig {
            enabled,
            routes,
            default_provider,
            default_model,
            allow_unlisted_models,
            quota_fault_mode,
            auth_scheme,
            inference_path_prefix,
            system_prompt_overrides,
            bridge_releases,
        }
    }
}
