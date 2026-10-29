//! Gateway configuration: on-disk spec and resolved runtime form.
//!
//! [`GatewayConfigSpec`] is the serde shape accepted under `gateway:` in the
//! services tree; [`GatewayConfig`] is its runtime projection. The gateway
//! owns no catalog: `default_model` and every route name catalog models and
//! endpoints, and are resolved against them by
//! [`ResolvedCatalog`](crate::services::catalog::ResolvedCatalog).
//!
//! A request that no declared route matched has no route id: access control
//! evaluates it against the `model/<id>` and `provider/<name>` entities that
//! match it at decision time.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod runtime;
mod validate;

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::ModelId;
use systemprompt_models::bridge::instance::{BridgeVersionWindow, InstanceBranding};

use crate::services::gateway::override_rule::SystemPromptRule;
use crate::services::gateway::route::GatewayRoute;

pub use runtime::GatewayConfig;

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
    pub default_model: Option<ModelId>,
    #[serde(default)]
    pub quota_fault_mode: QuotaFaultMode,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub require_scopes: Vec<systemprompt_identifiers::ScopeDimension>,
    #[serde(default = "default_true")]
    pub automatic_prompt_caching: bool,
    #[serde(default = "default_auth_scheme")]
    pub auth_scheme: String,
    #[serde(default = "default_inference_path_prefix")]
    pub inference_path_prefix: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub system_prompt_overrides: Vec<SystemPromptRule>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bridge_releases: Option<BridgeReleasesSpec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bridge_instance: Option<BridgeInstanceSpec>,
}

/// The routes and branding a shared bridge binary discovers for this
/// instance at `/.well-known/systemprompt-bridge`.
///
/// Both paths are relative to the gateway origin: the sign-in page and the
/// identity endpoint belong to the instance, not to the bridge build.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BridgeInstanceSpec {
    pub device_link_path: String,
    pub identity_path: String,
    #[serde(default)]
    pub branding: BridgeBrandingSpec,
}

/// Presentation the bridge shows for this instance before and after sign-in.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BridgeBrandingSpec {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logo_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme_css_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sign_in_hint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub docs_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contact_email: Option<String>,
}

impl BridgeBrandingSpec {
    #[must_use]
    pub fn to_branding(&self) -> InstanceBranding {
        InstanceBranding {
            display_name: self.display_name.clone(),
            logo_url: self.logo_url.clone(),
            theme_css_url: self.theme_css_url.clone(),
            sign_in_hint: self.sign_in_hint.clone(),
            docs_url: self.docs_url.clone(),
            contact_email: self.contact_email.clone(),
        }
    }
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub min_version: Option<semver::Version>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub max_version: Option<semver::Version>,
    #[serde(default)]
    pub enforce_max: bool,
}

impl BridgeReleasesSpec {
    #[must_use]
    pub fn api_base(&self) -> &str {
        self.api_base.as_deref().unwrap_or("https://api.github.com")
    }

    #[must_use]
    pub fn version_window(&self) -> BridgeVersionWindow {
        BridgeVersionWindow {
            min: self.min_version.clone(),
            max: self.max_version.clone(),
            enforce_max: self.enforce_max,
        }
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
            default_model: None,
            quota_fault_mode: QuotaFaultMode::default(),
            require_scopes: Vec::new(),
            automatic_prompt_caching: true,
            auth_scheme: default_auth_scheme(),
            inference_path_prefix: default_inference_path_prefix(),
            system_prompt_overrides: Vec::new(),
            bridge_releases: None,
            bridge_instance: None,
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
            default_model,
            quota_fault_mode,
            require_scopes,
            automatic_prompt_caching,
            auth_scheme,
            inference_path_prefix,
            system_prompt_overrides,
            bridge_releases,
            bridge_instance,
        } = self;

        GatewayConfig {
            enabled,
            routes,
            default_model,
            quota_fault_mode,
            require_scopes,
            automatic_prompt_caching,
            auth_scheme,
            inference_path_prefix,
            system_prompt_overrides,
            bridge_releases,
            bridge_instance,
        }
    }
}

const fn default_true() -> bool {
    true
}
