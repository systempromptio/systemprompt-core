//! Profile configuration module.

mod cloud;
mod database;
mod paths;
mod rate_limits;
mod runtime;
mod secrets;
mod security;
mod server;
mod site;
mod style;

pub use cloud::{CloudConfig, CloudValidationMode};
pub use database::DatabaseConfig;
pub use paths::{expand_home, resolve_path, resolve_with_home, PathsConfig};
pub use rate_limits::{
    default_agent_registry, default_agents, default_artifacts, default_burst, default_content,
    default_contexts, default_mcp, default_mcp_registry, default_oauth_auth, default_oauth_public,
    default_stream, default_tasks, RateLimitsConfig,
};
pub use runtime::{Environment, LogLevel, OutputFormat, RuntimeConfig};
pub use secrets::{SecretsConfig, SecretsSource, SecretsValidationMode};
pub use security::SecurityConfig;
pub use server::ServerConfig;
pub use site::SiteConfig;
pub use style::ProfileStyle;

use anyhow::{Context, Result};
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[allow(clippy::expect_used)]
static ENV_VAR_REGEX: Lazy<Regex> = Lazy::new(|| {
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,

    pub display_name: String,

    pub site: SiteConfig,

    pub database: DatabaseConfig,

    pub server: ServerConfig,

    pub paths: PathsConfig,

    pub security: SecurityConfig,

    pub rate_limits: RateLimitsConfig,

    #[serde(default)]
    pub runtime: RuntimeConfig,

    #[serde(default)]
    pub extensions: Option<HashMap<String, serde_json::Value>>,

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

    pub fn validate(&self) -> Result<()> {
        let mut errors: Vec<String> = Vec::new();
        let is_prod = self.runtime.environment.is_production();

        self.validate_required_fields(&mut errors);
        self.validate_required_paths(&mut errors, is_prod);
        self.validate_optional_paths(&mut errors, is_prod);
        self.validate_security_settings(&mut errors);

        if errors.is_empty() {
            Ok(())
        } else {
            anyhow::bail!(
                "Profile '{}' validation failed:\n  - {}",
                self.name,
                errors.join("\n  - ")
            )
        }
    }

    fn validate_required_fields(&self, errors: &mut Vec<String>) {
        Self::require_non_empty(errors, &self.name, "Profile name");
        Self::require_non_empty(errors, &self.display_name, "Profile display_name");
        Self::require_non_empty(errors, &self.site.name, "Site name");
        Self::require_non_empty(errors, &self.server.host, "Server host");
        Self::require_non_empty(errors, &self.server.api_server_url, "Server api_server_url");
        Self::require_non_empty(
            errors,
            &self.server.api_internal_url,
            "Server api_internal_url",
        );
        Self::require_non_empty(
            errors,
            &self.server.api_external_url,
            "Server api_external_url",
        );

        if self.server.port == 0 {
            errors.push("Server port must be greater than 0".to_string());
        }
    }

    fn require_non_empty(errors: &mut Vec<String>, value: &str, field_name: &str) {
        if value.is_empty() {
            errors.push(format!("{field_name} is required"));
        }
    }

    fn validate_required_paths(&self, errors: &mut Vec<String>, is_prod: bool) {
        Self::validate_required_path(errors, "system", &self.paths.system, is_prod);
        Self::validate_required_path(errors, "core", &self.paths.core, is_prod);

        if self.paths.services.is_empty() {
            errors.push("Paths services is required".to_string());
        } else if !Path::new(&self.paths.services).exists() {
            errors.push(format!(
                "Services path does not exist: {}",
                self.paths.services
            ));
        }
    }

    fn validate_required_path(errors: &mut Vec<String>, name: &str, path: &str, is_prod: bool) {
        if path.is_empty() {
            errors.push(format!("Paths {name} is required"));
            return;
        }

        if !Path::new(path).exists() {
            if is_prod {
                tracing::debug!(
                    "{} path does not exist (expected in production): {}",
                    name,
                    path
                );
            } else {
                errors.push(format!("{} path does not exist: {}", name, path));
            }
        }
    }

    fn validate_optional_paths(&self, errors: &mut Vec<String>, is_prod: bool) {
        Self::validate_optional_path(errors, "skills", &self.paths.skills, is_prod);
        Self::validate_optional_path(errors, "config", &self.paths.config, is_prod);
        Self::validate_optional_path(errors, "storage", &self.paths.storage, is_prod);
        Self::validate_optional_path(
            errors,
            "geoip_database",
            &self.paths.geoip_database,
            is_prod,
        );
        Self::validate_optional_path(
            errors,
            "content_config",
            &self.paths.content_config,
            is_prod,
        );
        Self::validate_optional_path(errors, "web_config", &self.paths.web_config, is_prod);
        Self::validate_optional_path(errors, "web_path", &self.paths.web_path, is_prod);
        Self::validate_optional_path(errors, "dockerfile", &self.paths.dockerfile, is_prod);
    }

    fn validate_security_settings(&self, errors: &mut Vec<String>) {
        if self.security.access_token_expiration <= 0 {
            errors.push("Security access_token_expiration must be positive".to_string());
        }

        if self.security.refresh_token_expiration <= 0 {
            errors.push("Security refresh_token_expiration must be positive".to_string());
        }
    }

    fn validate_optional_path(
        errors: &mut Vec<String>,
        name: &str,
        path: &Option<String>,
        is_production: bool,
    ) {
        if let Some(p) = path {
            if !p.is_empty() && !Path::new(p).exists() {
                if is_production {
                    tracing::debug!("Optional path '{}' does not exist: {}", name, p);
                } else {
                    errors.push(format!("paths.{} does not exist: {}", name, p));
                }
            }
        }
    }

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

    pub fn from_env(profile_name: &str, display_name: &str) -> Result<Self> {
        let require_env = |key: &str| -> Result<String> {
            std::env::var(key)
                .with_context(|| format!("Missing required environment variable: {}", key))
        };

        let db_type = Self::get_env("DATABASE_TYPE")
            .ok_or_else(|| anyhow::anyhow!("DATABASE_TYPE environment variable is required"))?;

        Ok(Self {
            name: profile_name.to_string(),
            display_name: display_name.to_string(),
            site: Self::site_config_from_env(&require_env)?,
            database: DatabaseConfig { db_type },
            server: Self::server_config_from_env(&require_env)?,
            paths: Self::paths_config_from_env(&require_env)?,
            security: Self::security_config_from_env()?,
            rate_limits: Self::rate_limits_from_env(),
            runtime: Self::runtime_config_from_env()?,
            extensions: None,
            cloud: None,
            secrets: None,
        })
    }

    fn get_env(key: &str) -> Option<String> {
        std::env::var(key).ok()
    }

    fn site_config_from_env(require_env: &dyn Fn(&str) -> Result<String>) -> Result<SiteConfig> {
        Ok(SiteConfig {
            name: require_env("SITENAME")?,
            github_link: Self::get_env("GITHUB_LINK"),
            service_display_name: Self::get_env("SERVICE_DISPLAY_NAME"),
            service_version: Self::get_env("SERVICE_VERSION"),
        })
    }

    fn server_config_from_env(
        require_env: &dyn Fn(&str) -> Result<String>,
    ) -> Result<ServerConfig> {
        Ok(ServerConfig {
            host: require_env("HOST")?,
            port: require_env("PORT")?.parse().context("Invalid PORT")?,
            api_server_url: require_env("API_SERVER_URL")?,
            api_internal_url: require_env("API_INTERNAL_URL")?,
            api_external_url: require_env("API_EXTERNAL_URL")?,
            use_https: Self::get_env("USE_HTTPS")
                .map(|v| v.to_lowercase() == "true")
                .unwrap_or(false),
            cors_allowed_origins: Self::get_env("CORS_ALLOWED_ORIGINS")
                .map(|s| s.split(',').map(|s| s.trim().to_string()).collect())
                .unwrap_or_default(),
        })
    }

    fn paths_config_from_env(require_env: &dyn Fn(&str) -> Result<String>) -> Result<PathsConfig> {
        Ok(PathsConfig {
            system: require_env("SYSTEM_PATH")?,
            core: require_env("CORE_PATH")?,
            services: require_env("SYSTEMPROMPT_SERVICES_PATH")?,
            skills: Self::get_env("SYSTEMPROMPT_SKILLS_PATH"),
            config: Self::get_env("SYSTEMPROMPT_CONFIG_PATH"),
            storage: Self::get_env("STORAGE_PATH"),
            cargo_target: Self::get_env("CARGO_TARGET_DIR"),
            binary_dir: Self::get_env("SYSTEMPROMPT_BINARY_DIR"),
            geoip_database: Self::get_env("GEOIP_DATABASE_PATH"),
            ai_config: Self::get_env("AI_CONFIG_PATH"),
            content_config: Self::get_env("CONTENT_CONFIG_PATH"),
            web_config: Self::get_env("SYSTEMPROMPT_WEB_CONFIG_PATH"),
            web_metadata: Self::get_env("SYSTEMPROMPT_WEB_METADATA_PATH"),
            web_path: Self::get_env("SYSTEMPROMPT_WEB_PATH"),
            scg_templates: Self::get_env("SCG_TEMPLATES_PATH"),
            scg_assets: Self::get_env("SCG_ASSETS_PATH"),
            dockerfile: Self::get_env("SYSTEMPROMPT_DOCKERFILE_PATH"),
            web_dist: Self::get_env("SYSTEMPROMPT_WEB_DIST_PATH"),
        })
    }

    fn security_config_from_env() -> Result<SecurityConfig> {
        use crate::auth::JwtAudience;

        let issuer = Self::get_env("JWT_ISSUER")
            .ok_or_else(|| anyhow::anyhow!("JWT_ISSUER environment variable is required"))?;

        let access_token_expiration = Self::get_env("JWT_ACCESS_TOKEN_EXPIRATION")
            .ok_or_else(|| {
                anyhow::anyhow!("JWT_ACCESS_TOKEN_EXPIRATION environment variable is required")
            })?
            .parse()
            .map_err(|e| anyhow::anyhow!("Failed to parse JWT_ACCESS_TOKEN_EXPIRATION: {e}"))?;

        let refresh_token_expiration = Self::get_env("JWT_REFRESH_TOKEN_EXPIRATION")
            .ok_or_else(|| {
                anyhow::anyhow!("JWT_REFRESH_TOKEN_EXPIRATION environment variable is required")
            })?
            .parse()
            .map_err(|e| anyhow::anyhow!("Failed to parse JWT_REFRESH_TOKEN_EXPIRATION: {e}"))?;

        let audiences = Self::get_env("JWT_AUDIENCES")
            .ok_or_else(|| anyhow::anyhow!("JWT_AUDIENCES environment variable is required"))?
            .split(',')
            .map(|s| s.trim().parse::<JwtAudience>())
            .collect::<Result<Vec<_>>>()?;

        Ok(SecurityConfig {
            issuer,
            access_token_expiration,
            refresh_token_expiration,
            audiences,
        })
    }

    fn rate_limits_from_env() -> RateLimitsConfig {
        let parse_rate = |key: &str, default: fn() -> u64| -> u64 {
            Self::get_env(key)
                .and_then(|s| {
                    s.parse().map_err(|e| {
                        tracing::warn!(key = %key, value = %s, error = %e, "Failed to parse rate limit value");
                        e
                    }).ok()
                })
                .unwrap_or_else(default)
        };

        RateLimitsConfig {
            disabled: Self::get_env("RATE_LIMIT_DISABLED")
                .map(|v| v.to_lowercase() == "true")
                .unwrap_or(false),
            oauth_public_per_second: parse_rate(
                "RATE_LIMIT_OAUTH_PUBLIC_PER_SECOND",
                default_oauth_public,
            ),
            oauth_auth_per_second: parse_rate(
                "RATE_LIMIT_OAUTH_AUTH_PER_SECOND",
                default_oauth_auth,
            ),
            contexts_per_second: parse_rate("RATE_LIMIT_CONTEXTS_PER_SECOND", default_contexts),
            tasks_per_second: parse_rate("RATE_LIMIT_TASKS_PER_SECOND", default_tasks),
            artifacts_per_second: parse_rate("RATE_LIMIT_ARTIFACTS_PER_SECOND", default_artifacts),
            agent_registry_per_second: parse_rate(
                "RATE_LIMIT_AGENT_REGISTRY_PER_SECOND",
                default_agent_registry,
            ),
            agents_per_second: parse_rate("RATE_LIMIT_AGENTS_PER_SECOND", default_agents),
            mcp_registry_per_second: parse_rate(
                "RATE_LIMIT_MCP_REGISTRY_PER_SECOND",
                default_mcp_registry,
            ),
            mcp_per_second: parse_rate("RATE_LIMIT_MCP_PER_SECOND", default_mcp),
            stream_per_second: parse_rate("RATE_LIMIT_STREAM_PER_SECOND", default_stream),
            content_per_second: parse_rate("RATE_LIMIT_CONTENT_PER_SECOND", default_content),
            burst_multiplier: parse_rate("RATE_LIMIT_BURST_MULTIPLIER", default_burst),
        }
    }

    fn runtime_config_from_env() -> Result<RuntimeConfig> {
        let parse_or_default = |key: &str, default: &str| -> Result<String> {
            Ok(Self::get_env(key).unwrap_or_else(|| default.to_string()))
        };

        Ok(RuntimeConfig {
            environment: parse_or_default("SYSTEMPROMPT_ENV", "development")?
                .parse()
                .map_err(|e| anyhow::anyhow!("{}", e))?,
            log_level: parse_or_default("SYSTEMPROMPT_LOG_LEVEL", "normal")?
                .parse()
                .map_err(|e| anyhow::anyhow!("{}", e))?,
            output_format: parse_or_default("SYSTEMPROMPT_OUTPUT_FORMAT", "text")?
                .parse()
                .map_err(|e| anyhow::anyhow!("{}", e))?,
            no_color: Self::get_env("NO_COLOR").is_some(),
            non_interactive: Self::get_env("CI").is_some(),
        })
    }

    pub fn save(&self, services_path: &Path) -> Result<()> {
        let profiles_dir = services_path.join("profiles");
        std::fs::create_dir_all(&profiles_dir).context("Failed to create profiles directory")?;

        let profile_path = profiles_dir.join(format!("{}.secrets.profile.yaml", self.name));
        let content = serde_yaml::to_string(self).context("Failed to serialize profile")?;

        let content_with_header = format!(
            "# SystemPrompt Profile: {}\n# \n# WARNING: This file contains secrets (API keys, JWT \
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
                let prefix = &url[..colon_pos + 1];
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
