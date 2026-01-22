//! Profile configuration module.

mod cloud;
mod database;
mod from_env;
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
pub use paths::{expand_home, resolve_path, resolve_with_home, PathsConfig};
pub use rate_limits::{
    default_a2a_multiplier, default_admin_multiplier, default_agent_registry, default_agents,
    default_anon_multiplier, default_artifacts, default_burst, default_content, default_contexts,
    default_mcp, default_mcp_multiplier, default_mcp_registry, default_oauth_auth,
    default_oauth_public, default_service_multiplier, default_stream, default_tasks,
    default_user_multiplier, RateLimitsConfig, TierMultipliers,
};
pub use runtime::{Environment, LogLevel, OutputFormat, RuntimeConfig};
pub use secrets::{SecretsConfig, SecretsSource, SecretsValidationMode};
pub use security::SecurityConfig;
pub use server::ServerConfig;
pub use site::SiteConfig;
pub use style::ProfileStyle;

use anyhow::{Context, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

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
}

impl Profile {
    pub fn parse(content: &str, profile_path: &Path) -> Result<Self> {
        let content = substitute_env_vars(content);

        let mut profile: Self = serde_yaml::from_str(&content)
            .with_context(|| format!("Failed to parse profile: {}", profile_path.display()))?;

        let profile_dir = profile_path
            .parent()
            .with_context(|| format!("Invalid profile path: {}", profile_path.display()))?;

        profile.paths.resolve_relative_to(profile_dir);

        Ok(profile)
    }

    pub fn to_yaml(&self) -> Result<String> {
        serde_yaml::to_string(self).context("Failed to serialize profile")
    }

    // NOTE: validate() is implemented in validation.rs

    pub fn list_available(services_path: &Path) -> Vec<String> {
        let profiles_dir = services_path.join("profiles");
        if !profiles_dir.exists() {
            return Vec::new();
        }

        std::fs::read_dir(&profiles_dir)
            .map(|entries| {
                entries
                    .filter_map(std::result::Result::ok)
                    .filter_map(|e| {
                        let name = e.file_name().to_string_lossy().to_string();
                        if name.ends_with(".secrets.profile.yaml") {
                            Some(name.trim_end_matches(".secrets.profile.yaml").to_string())
                        } else {
                            None
                        }
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    // NOTE: from_env() is implemented in from_env.rs

    pub fn save(&self, services_path: &Path) -> Result<()> {
        let profiles_dir = services_path.join("profiles");
        std::fs::create_dir_all(&profiles_dir).context("Failed to create profiles directory")?;

        let profile_path = profiles_dir.join(format!("{}.secrets.profile.yaml", self.name));
        let content = serde_yaml::to_string(self).context("Failed to serialize profile")?;

        let content_with_header = format!(
            "# systemprompt.io Profile: {}\n# \n# WARNING: This file contains secrets (API keys, JWT \
             secrets, database credentials).\n# DO NOT commit this file to version control.\n# DO \
             NOT share this file publicly.\n# \n# Generated from environment variables\n\n{}",
            self.display_name, content
        );

        std::fs::write(&profile_path, content_with_header)
            .with_context(|| format!("Failed to write profile file: {}", profile_path.display()))?;

        Ok(())
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

    pub fn credentials_path(&self, profile_dir: Option<&Path>) -> Result<PathBuf> {
        let cloud = self
            .cloud
            .as_ref()
            .context("Profile missing cloud configuration")?;
        Ok(Self::resolve_cloud_path(
            &cloud.credentials_path,
            profile_dir,
        ))
    }

    pub fn tenants_path(&self, profile_dir: Option<&Path>) -> Result<PathBuf> {
        let cloud = self
            .cloud
            .as_ref()
            .context("Profile missing cloud configuration")?;
        Ok(Self::resolve_cloud_path(&cloud.tenants_path, profile_dir))
    }

    fn resolve_cloud_path(path_str: &str, profile_dir: Option<&Path>) -> PathBuf {
        profile_dir.map_or_else(
            || expand_home(path_str),
            |base| resolve_with_home(base, path_str),
        )
    }
}
