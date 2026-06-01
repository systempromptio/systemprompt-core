//! Profile configuration models — the deserialized shape of a
//! `.systemprompt/profiles/<name>/profile.yaml` document.
//!
//! Covers server, database, paths, secrets, security, rate limits,
//! gateway, governance, and runtime sections, plus validation rules and
//! environment-variable interpolation.

mod cloud;
mod database;
mod error;
mod from_env;
mod gateway;
mod governance;
mod info;
mod paths;
mod providers;
mod rate_limits;
mod runtime;
mod secrets;
mod security;
mod server;
mod site;
mod style;
mod validation;

pub use cloud::{CloudConfig, CloudValidationMode};
pub use database::DatabaseConfig;
pub use error::{ProfileError, ProfileResult};
pub use gateway::{
    GatewayConfig, GatewayConfigSpec, GatewayProfileError, GatewayResult, GatewayRoute,
    GatewayState, slugify_pattern, synthesize_route_id,
};
pub use governance::{
    AuthzConfig, AuthzHookConfig, AuthzMode, GovernanceConfig, UNRESTRICTED_ACKNOWLEDGEMENT,
};
pub use info::ProfileInfo;
pub use paths::{PathsConfig, expand_home, resolve_path, resolve_with_home};
pub use providers::{
    ProviderEntry, ProviderModel, ProviderRegistry, ProviderRegistryError, ProviderRegistryResult,
    WireProtocol,
};
pub use rate_limits::{
    RateLimitsConfig, TierMultipliers, default_a2a_multiplier, default_admin_multiplier,
    default_agent_registry, default_agents, default_anon_multiplier, default_artifacts,
    default_burst, default_content, default_contexts, default_mcp, default_mcp_multiplier,
    default_mcp_registry, default_oauth_auth, default_oauth_public, default_service_multiplier,
    default_stream, default_tasks, default_user_multiplier,
};
pub use runtime::{Environment, LogLevel, OutputFormat, RuntimeConfig};
pub use secrets::{SecretsConfig, SecretsSource, SecretsValidationMode};
pub use security::{
    GATEWAY_REQUIRED_RESOURCE_AUDIENCES, SecurityConfig, TrustedIssuer, default_resource_audiences,
};
pub use server::{ContentNegotiationConfig, SecurityHeadersConfig, ServerConfig};
pub use site::SiteConfig;
pub use style::ProfileStyle;

use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::env::{interpolate, read_env_optional};

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExtensionsConfig {
    #[serde(default)]
    pub disabled: Vec<String>,
}

impl ExtensionsConfig {
    pub fn is_disabled(&self, extension_id: &str) -> bool {
        self.disabled.iter().any(|id| id == extension_id)
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, schemars::JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum ProfileType {
    #[default]
    Local,
    Cloud,
}

impl ProfileType {
    pub const fn is_cloud(&self) -> bool {
        matches!(self, Self::Cloud)
    }

    pub const fn is_local(&self) -> bool {
        matches!(self, Self::Local)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub name: String,

    pub display_name: String,

    #[serde(default)]
    pub target: ProfileType,

    pub site: SiteConfig,

    pub database: DatabaseConfig,

    pub server: ServerConfig,

    pub paths: PathsConfig,

    pub security: SecurityConfig,

    pub rate_limits: RateLimitsConfig,

    pub system_admin: crate::services::SystemAdminConfig,

    #[serde(default)]
    pub runtime: RuntimeConfig,

    #[serde(default)]
    pub cloud: Option<CloudConfig>,

    #[serde(default)]
    pub secrets: Option<SecretsConfig>,

    #[serde(default)]
    pub extensions: ExtensionsConfig,

    #[serde(default)]
    pub providers: ProviderRegistry,

    #[serde(default)]
    pub gateway: Option<GatewayState>,

    #[serde(default)]
    pub governance: Option<GovernanceConfig>,
}

impl Profile {
    #[must_use]
    pub fn is_local_trial(&self) -> bool {
        self.cloud.as_ref().is_none_or(CloudConfig::is_local_trial)
    }

    pub fn from_yaml(content: &str, profile_path: &Path) -> ProfileResult<Self> {
        let content = interpolate(content, &|name| read_env_optional(name));

        let mut profile: Self =
            serde_yaml::from_str(&content).map_err(|source| ProfileError::ParseYaml {
                path: profile_path.to_path_buf(),
                source,
            })?;

        let profile_dir =
            profile_path
                .parent()
                .ok_or_else(|| ProfileError::InvalidProfilePath {
                    path: profile_path.to_path_buf(),
                })?;

        profile.paths.resolve_relative_to(profile_dir);

        Ok(profile)
    }

    pub fn to_yaml(&self) -> ProfileResult<String> {
        serde_yaml::to_string(self).map_err(ProfileError::SerializeYaml)
    }

    pub fn profile_style(&self) -> ProfileStyle {
        match self.name.to_lowercase().as_str() {
            "dev" | "development" | "local" => ProfileStyle::Development,
            "prod" | "production" => ProfileStyle::Production,
            "staging" | "stage" => ProfileStyle::Staging,
            "test" | "testing" => ProfileStyle::Test,
            _ => ProfileStyle::Custom,
        }
    }

    pub fn mask_secret(value: &str, visible_chars: usize) -> String {
        if value.is_empty() {
            return "(not set)".to_owned();
        }
        if value.len() <= visible_chars {
            return "***".to_owned();
        }
        format!("{}...", &value[..visible_chars])
    }

    pub fn mask_database_url(url: &str) -> String {
        if let Some(at_pos) = url.find('@') {
            if let Some(colon_pos) = url[..at_pos].rfind(':') {
                let prefix = &url[..=colon_pos];
                let suffix = &url[at_pos..];
                return format!("{}***{}", prefix, suffix);
            }
        }
        url.to_owned()
    }

    pub fn is_masked_database_url(url: &str) -> bool {
        url.contains(":***@") || url.contains(":********@")
    }
}
