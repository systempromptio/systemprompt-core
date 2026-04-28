mod cloud;
mod database;
mod error;
mod from_env;
mod gateway;
mod info;
mod paths;
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
    GatewayCatalog, GatewayConfig, GatewayModel, GatewayProfileError, GatewayProvider,
    GatewayResult, GatewayRoute,
};
pub use info::ProfileInfo;
pub use paths::{PathsConfig, expand_home, resolve_path, resolve_with_home};
pub use rate_limits::{
    RateLimitsConfig, TierMultipliers, default_a2a_multiplier, default_admin_multiplier,
    default_agent_registry, default_agents, default_anon_multiplier, default_artifacts,
    default_burst, default_content, default_contexts, default_mcp, default_mcp_multiplier,
    default_mcp_registry, default_oauth_auth, default_oauth_public, default_service_multiplier,
    default_stream, default_tasks, default_user_multiplier,
};
pub use runtime::{Environment, LogLevel, OutputFormat, RuntimeConfig};
pub use secrets::{SecretsConfig, SecretsSource, SecretsValidationMode};
pub use security::SecurityConfig;
pub use server::{ContentNegotiationConfig, SecurityHeadersConfig, ServerConfig};
pub use site::SiteConfig;
pub use style::ProfileStyle;

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::LazyLock;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExtensionsConfig {
    #[serde(default)]
    pub disabled: Vec<String>,
}

impl ExtensionsConfig {
    pub fn is_disabled(&self, extension_id: &str) -> bool {
        self.disabled.iter().any(|id| id == extension_id)
    }
}

#[allow(clippy::expect_used)]
static ENV_VAR_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\$\{(\w+)\}")
        .expect("ENV_VAR_REGEX is a valid regex - this is a compile-time constant")
});

fn env_var_regex() -> &'static Regex {
    &ENV_VAR_REGEX
}

fn substitute_env_vars(content: &str) -> String {
    env_var_regex()
        .replace_all(content, |caps: &regex::Captures| {
            let var_name = &caps[1];
            std::env::var(var_name).unwrap_or_else(|_| caps[0].to_string())
        })
        .to_string()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

    #[serde(default)]
    pub runtime: RuntimeConfig,

    #[serde(default)]
    pub cloud: Option<CloudConfig>,

    #[serde(default)]
    pub secrets: Option<SecretsConfig>,

    #[serde(default)]
    pub extensions: ExtensionsConfig,

    #[serde(default)]
    pub gateway: Option<GatewayConfig>,
}

impl Profile {
    #[must_use]
    pub fn is_local_trial(&self) -> bool {
        self.cloud.as_ref().is_none_or(CloudConfig::is_local_trial)
    }

    pub fn parse(content: &str, profile_path: &Path) -> ProfileResult<Self> {
        let content = substitute_env_vars(content);

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

        if let Some(gateway) = profile.gateway.as_mut() {
            gateway.resolve_catalog(profile_dir)?;
        }

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
            return "(not set)".to_string();
        }
        if value.len() <= visible_chars {
            return "***".to_string();
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
        url.to_string()
    }

    pub fn is_masked_database_url(url: &str) -> bool {
        url.contains(":***@") || url.contains(":********@")
    }
}
