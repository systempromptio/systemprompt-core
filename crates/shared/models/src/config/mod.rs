use std::sync::OnceLock;
use systemprompt_traits::ConfigProvider;

use crate::auth::JwtAudience;
use crate::profile::{ContentNegotiationConfig, SecurityHeadersConfig};

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

#[derive(Debug, Clone)]
pub struct Config {
    pub sitename: String,
    pub database_type: String,
    pub database_url: String,
    pub database_write_url: Option<String>,
    pub github_link: String,
    pub github_token: Option<String>,
    pub system_path: String,
    pub services_path: String,
    pub bin_path: String,
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
    pub is_cloud: bool,
    pub content_negotiation: ContentNegotiationConfig,
    pub security_headers: SecurityHeadersConfig,
    pub allow_registration: bool,
}

impl Config {
    pub fn is_initialized() -> bool {
        CONFIG.get().is_some()
    }

    pub fn get() -> anyhow::Result<&'static Self> {
        CONFIG
            .get()
            .ok_or_else(|| anyhow::anyhow!("Config not initialized. Call Config::init() first."))
    }

    pub fn install(config: Self) -> Result<(), Box<Self>> {
        CONFIG.set(config).map_err(Box::new)
    }
}

impl ConfigProvider for Config {
    fn get(&self, key: &str) -> Option<String> {
        match key {
            "database_type" => Some(self.database_type.clone()),
            "database_url" => Some(self.database_url.clone()),
            "database_write_url" => self.database_write_url.clone(),
            "host" => Some(self.host.clone()),
            "port" => Some(self.port.to_string()),
            "system_path" => Some(self.system_path.clone()),
            "services_path" => Some(self.services_path.clone()),
            "bin_path" => Some(self.bin_path.clone()),
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
            "is_cloud" => Some(self.is_cloud.to_string()),
            _ => None,
        }
    }

    fn database_url(&self) -> &str {
        &self.database_url
    }

    fn database_write_url(&self) -> Option<&str> {
        self.database_write_url.as_deref()
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
