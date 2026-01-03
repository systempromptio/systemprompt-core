use anyhow::Result;
use std::sync::OnceLock;
use systemprompt_traits::ConfigProvider;

use crate::auth::JwtAudience;
use crate::profile::Profile;
use crate::profile_bootstrap::ProfileBootstrap;
use crate::secrets::SecretsBootstrap;

mod environment;
mod paths;
mod rate_limits;
mod validation;
mod verbosity;

pub use environment::Environment;
pub use paths::PathNotConfiguredError;
pub use rate_limits::RateLimitConfig;
pub use validation::{
    format_path_errors, validate_optional_path, validate_postgres_url, validate_profile_paths,
    validate_required_optional_path, validate_required_path,
};
pub use verbosity::VerbosityLevel;

static CONFIG: OnceLock<Config> = OnceLock::new();

#[allow(clippy::struct_field_names)]
struct BuildConfigPaths {
    system_path: String,
    skills_path: String,
    settings_path: String,
    content_config_path: String,
    web_path: String,
    web_config_path: String,
    web_metadata_path: String,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub sitename: String,
    pub database_type: String,
    pub database_url: String,
    pub github_link: String,
    pub github_token: Option<String>,
    pub system_path: String,
    pub services_path: String,
    pub skills_path: String,
    pub settings_path: String,
    pub content_config_path: String,
    pub geoip_database_path: Option<String>,
    pub web_path: String,
    pub web_config_path: String,
    pub web_metadata_path: String,
    pub host: String,
    pub port: u16,
    pub api_server_url: String,
    pub api_internal_url: String,
    pub api_external_url: String,
    pub jwt_issuer: String,
    pub jwt_access_token_expiration: i64,
    pub jwt_refresh_token_expiration: i64,
    pub jwt_audiences: Vec<JwtAudience>,
    pub use_https: bool,
    pub rate_limits: RateLimitConfig,
    pub cors_allowed_origins: Vec<String>,
}

impl Config {
    pub fn is_initialized() -> bool {
        CONFIG.get().is_some()
    }

    pub fn init() -> Result<()> {
        let profile = ProfileBootstrap::get()
            .map_err(|e| anyhow::anyhow!("Profile not initialized: {}", e))?;

        let config = Self::from_profile(profile)?;
        CONFIG
            .set(config)
            .map_err(|_| anyhow::anyhow!("Config already initialized"))?;
        Ok(())
    }

    pub fn try_init() -> Result<()> {
        if Self::is_initialized() {
            return Ok(());
        }
        Self::init()
    }

    pub fn get() -> Result<&'static Self> {
        CONFIG
            .get()
            .ok_or_else(|| anyhow::anyhow!("Config not initialized. Call Config::init() first."))
    }

    pub fn from_profile(profile: &Profile) -> Result<Self> {
        let profile_path = ProfileBootstrap::get_path()
            .map(ToString::to_string)
            .unwrap_or_else(|_| "<not set>".to_string());

        let path_report = validate_profile_paths(profile, &profile_path);
        if path_report.has_errors() {
            return Err(anyhow::anyhow!(
                "{}",
                format_path_errors(&path_report, &profile_path)
            ));
        }

        let system_path = Self::canonicalize_path(&profile.paths.system, "system")?;

        let skills_path = profile.paths.skills();
        let settings_path =
            Self::require_yaml_path("config", Some(&profile.paths.config()), &profile_path)?;
        let content_config_path = Self::require_yaml_path(
            "content_config",
            Some(&profile.paths.content_config()),
            &profile_path,
        )?;
        let web_path = profile.paths.web_path_resolved();
        let web_config_path = Self::require_yaml_path(
            "web_config",
            Some(&profile.paths.web_config()),
            &profile_path,
        )?;
        let web_metadata_path = Self::require_yaml_path(
            "web_metadata",
            Some(&profile.paths.web_metadata()),
            &profile_path,
        )?;

        let paths = BuildConfigPaths {
            system_path,
            skills_path,
            settings_path,
            content_config_path,
            web_path,
            web_config_path,
            web_metadata_path,
        };
        let config = Self::build_config(profile, paths)?;

        config.validate_database_config()?;
        Ok(config)
    }

    fn canonicalize_path(path: &str, name: &str) -> Result<String> {
        std::fs::canonicalize(path)
            .map(|p| p.to_string_lossy().to_string())
            .map_err(|e| anyhow::anyhow!("Failed to canonicalize {} path: {}", name, e))
    }

    fn require_yaml_path(field: &str, value: Option<&str>, profile_path: &str) -> Result<String> {
        let path =
            value.ok_or_else(|| anyhow::anyhow!("Missing required path: paths.{}", field))?;

        let content = std::fs::read_to_string(path).map_err(|e| {
            anyhow::anyhow!(
                "Profile Error: Cannot read file\n\n  Field: paths.{}\n  Path: {}\n  Error: {}\n  \
                 Profile: {}",
                field,
                path,
                e,
                profile_path
            )
        })?;

        serde_yaml::from_str::<serde_yaml::Value>(&content).map_err(|e| {
            anyhow::anyhow!(
                "Profile Error: Invalid YAML syntax\n\n  Field: paths.{}\n  Path: {}\n  Error: \
                 {}\n  Profile: {}",
                field,
                path,
                e,
                profile_path
            )
        })?;

        Ok(path.to_string())
    }

    fn build_config(profile: &Profile, paths: BuildConfigPaths) -> Result<Self> {
        Ok(Self {
            sitename: profile.site.name.clone(),
            database_type: profile.database.db_type.clone(),
            database_url: Self::resolve_database_url(profile)?,
            github_link: profile
                .site
                .github_link
                .clone()
                .unwrap_or_else(|| "https://github.com/systemprompt/systemprompt-os".to_string()),
            github_token: None,
            system_path: paths.system_path,
            services_path: profile.paths.services.clone(),
            skills_path: paths.skills_path,
            settings_path: paths.settings_path,
            content_config_path: paths.content_config_path,
            geoip_database_path: profile.paths.geoip_database.clone(),
            web_path: paths.web_path,
            web_config_path: paths.web_config_path,
            web_metadata_path: paths.web_metadata_path,
            host: profile.server.host.clone(),
            port: profile.server.port,
            api_server_url: profile.server.api_server_url.clone(),
            api_internal_url: profile.server.api_internal_url.clone(),
            api_external_url: profile.server.api_external_url.clone(),
            jwt_issuer: profile.security.issuer.clone(),
            jwt_access_token_expiration: profile.security.access_token_expiration,
            jwt_refresh_token_expiration: profile.security.refresh_token_expiration,
            jwt_audiences: profile.security.audiences.clone(),
            use_https: profile.server.use_https,
            rate_limits: (&profile.rate_limits).into(),
            cors_allowed_origins: profile.server.cors_allowed_origins.clone(),
        })
    }

    /// Initialize configuration from a profile.
    ///
    /// This loads the profile and sets it as the global configuration.
    pub fn init_from_profile(profile: &Profile) -> Result<()> {
        let config = Self::from_profile(profile)?;
        CONFIG
            .set(config)
            .map_err(|_| anyhow::anyhow!("Config already initialized"))?;
        Ok(())
    }

    /// Get database URL from secrets (required).
    fn resolve_database_url(_profile: &Profile) -> Result<String> {
        let secrets = SecretsBootstrap::get().map_err(|_| {
            anyhow::anyhow!(
                "Secrets not initialized. Database URL must be configured in secrets.json or via \
                 DATABASE_URL environment variable."
            )
        })?;
        Ok(secrets.database_url.clone())
    }

    pub fn validate_database_config(&self) -> Result<()> {
        let db_type = self.database_type.to_lowercase();

        if db_type != "postgres" && db_type != "postgresql" {
            return Err(anyhow::anyhow!(
                "Unsupported database type '{}'. Only 'postgres' is supported.",
                self.database_type
            ));
        }

        validate_postgres_url(&self.database_url)?;
        Ok(())
    }
}

impl ConfigProvider for Config {
    fn get(&self, key: &str) -> Option<String> {
        match key {
            "database_type" => Some(self.database_type.clone()),
            "database_url" => Some(self.database_url.clone()),
            "host" => Some(self.host.clone()),
            "port" => Some(self.port.to_string()),
            "system_path" => Some(self.system_path.clone()),
            "services_path" => Some(self.services_path.clone()),
            "skills_path" => Some(self.skills_path.clone()),
            "settings_path" => Some(self.settings_path.clone()),
            "content_config_path" => Some(self.content_config_path.clone()),
            "web_path" => Some(self.web_path.clone()),
            "web_config_path" => Some(self.web_config_path.clone()),
            "web_metadata_path" => Some(self.web_metadata_path.clone()),
            "sitename" => Some(self.sitename.clone()),
            "github_link" => Some(self.github_link.clone()),
            "github_token" => self.github_token.clone(),
            "api_server_url" => Some(self.api_server_url.clone()),
            "api_external_url" => Some(self.api_external_url.clone()),
            "jwt_issuer" => Some(self.jwt_issuer.clone()),
            _ => None,
        }
    }

    fn database_url(&self) -> &str {
        &self.database_url
    }

    fn system_path(&self) -> &str {
        &self.system_path
    }

    fn api_port(&self) -> u16 {
        self.port
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
