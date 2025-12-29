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
pub use paths::{PathNotConfiguredError, SystemPaths};
pub use rate_limits::RateLimitConfig;
pub use validation::{
    format_path_errors, validate_optional_path, validate_postgres_url, validate_profile_paths,
    validate_required_optional_path, validate_required_path,
};
pub use verbosity::VerbosityLevel;

static CONFIG: OnceLock<Config> = OnceLock::new();

struct BuildConfigPaths {
    system_path: String,
    core_path: String,
    cargo_target_dir: String,
    binary_dir: Option<String>,
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
    pub core_path: String,
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
    pub cargo_target_dir: String,
    pub binary_dir: Option<String>,
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
        let core_path = Self::canonicalize_path(&profile.paths.core, "core")?;
        let cargo_target_dir =
            Self::resolve_target_dir(profile.paths.cargo_target.as_deref(), &system_path);
        let binary_dir =
            Self::resolve_optional_path(profile.paths.binary_dir.as_deref(), &system_path);

        let skills_path = Self::require_path(profile.paths.skills.as_deref(), "skills")?;
        let settings_path =
            Self::require_yaml_path("config", profile.paths.config.as_deref(), &profile_path)?;
        let content_config_path = Self::require_yaml_path(
            "content_config",
            profile.paths.content_config.as_deref(),
            &profile_path,
        )?;
        let web_path = Self::require_path(profile.paths.web_path.as_deref(), "web_path")?;
        let web_config_path = Self::require_yaml_path(
            "web_config",
            profile.paths.web_config.as_deref(),
            &profile_path,
        )?;
        let web_metadata_path = Self::require_yaml_path(
            "web_metadata",
            profile.paths.web_metadata.as_deref(),
            &profile_path,
        )?;

        let paths = BuildConfigPaths {
            system_path,
            core_path,
            cargo_target_dir,
            binary_dir,
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

    fn resolve_target_dir(cargo_target: Option<&str>, system_path: &str) -> String {
        cargo_target
            .map(|dir| Self::make_absolute(dir, system_path))
            .unwrap_or_else(|| {
                std::path::Path::new(system_path)
                    .join("target")
                    .to_string_lossy()
                    .to_string()
            })
    }

    fn resolve_optional_path(path: Option<&str>, system_path: &str) -> Option<String> {
        path.map(|dir| Self::make_absolute(dir, system_path))
    }

    fn make_absolute(dir: &str, base: &str) -> String {
        if std::path::Path::new(dir).is_absolute() {
            dir.to_string()
        } else {
            std::path::Path::new(base)
                .join(dir)
                .to_string_lossy()
                .to_string()
        }
    }

    fn require_path(value: Option<&str>, field: &str) -> Result<String> {
        value
            .map(ToString::to_string)
            .ok_or_else(|| anyhow::anyhow!("Missing required path: paths.{}", field))
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
            core_path: paths.core_path,
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
            cargo_target_dir: paths.cargo_target_dir,
            binary_dir: paths.binary_dir,
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
            "core_path" => Some(self.core_path.clone()),
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
            "cargo_target_dir" => Some(self.cargo_target_dir.clone()),
            "binary_dir" => self.binary_dir.clone(),
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
