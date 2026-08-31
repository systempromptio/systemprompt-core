//! Gateway configuration: on-disk spec and resolved runtime form.
//!
//! [`GatewayConfigSpec`] is the serde shape accepted under `gateway:` in a
//! profile; [`GatewayConfig`] is its runtime projection. Routes carry no
//! embedded provider catalog — every route resolves its provider against
//! `profile.providers` (`ProviderRegistry`) at use time.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod runtime;
mod validate;

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::ProviderId;

use crate::profile::gateway::override_rule::SystemPromptRule;
use crate::profile::gateway::route::GatewayRoute;

pub use runtime::GatewayConfig;

pub(crate) const DEFAULT_ROUTE_PATTERN: &str = "*";

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GatewayConfigSpec {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub routes: Vec<GatewayRoute>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_provider: Option<ProviderId>,
    /// Model a freshly-installed bridge client selects when the user has not
    /// chosen one. Advertised over `GET /v1/bridge/profile`; changing it here
    /// moves the fleet default without shipping a new bridge build.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_model: Option<String>,
    #[serde(default)]
    pub allow_unlisted_models: bool,
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
    pub token_env: Option<String>,
    #[serde(default = "default_tag_prefix")]
    pub tag_prefix: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pinned_version: Option<String>,
    #[serde(default)]
    pub assets: std::collections::BTreeMap<String, String>,
    // Why: the GitHub API host is a field rather than a constant so the
    // release routes can be pointed at a stub. Hardcoded, every line past
    // "is this configured" needed a real call to api.github.com to reach,
    // which is not something a test can do. Absent means the real host, so
    // no deployment has to know this exists.
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
            auth_scheme,
            inference_path_prefix,
            system_prompt_overrides,
            bridge_releases,
        }
    }
}
