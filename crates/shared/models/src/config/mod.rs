//! Global runtime [`Config`] singleton and validation helpers.
//!
//! [`Config`] is the resolved, flat configuration installed once at
//! startup into a process-wide `OnceLock` and read via [`Config::get`].
//! Submodules cover postgres-URL validation and rate-limit shapes.
//! Accessors return [`crate::errors::ConfigError`] when not initialized.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;
use std::sync::OnceLock;
use systemprompt_traits::ConfigProvider;

use crate::auth::JwtAudience;
use crate::profile::{ContentNegotiationConfig, SecurityHeadersConfig, TrustedIssuer};

mod paths;
mod rate_limits;
mod validation;

pub use paths::PathNotConfiguredError;
pub use rate_limits::RateLimitConfig;
pub use validation::validate_postgres_url;

static CONFIG: OnceLock<Config> = OnceLock::new();

pub const DEFAULT_MAX_CONCURRENT_STREAMS: usize = 256;

#[must_use]
pub fn stable_instance_id(lookup: impl Fn(&str) -> Option<String>) -> Option<String> {
    lookup("HOSTNAME")
        .map(|h| h.trim().to_owned())
        .filter(|h| !h.is_empty())
}

#[must_use]
pub fn random_instance_id() -> String {
    format!("instance-{}", uuid::Uuid::new_v4().simple())
}

#[derive(Clone)]
pub struct Config {
    pub instance_id: String,
    pub metrics_port: Option<u16>,
    pub max_concurrent_streams: usize,
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
    pub allowed_resource_audiences: Vec<String>,
    pub trusted_issuers: Vec<TrustedIssuer>,
    pub id_jag_ttl_secs: i64,
    pub signing_key_path: PathBuf,
    pub use_https: bool,
    pub rate_limits: RateLimitConfig,
    pub cors_allowed_origins: Vec<String>,
    pub trusted_proxies: Vec<ipnet::IpNet>,
    pub is_cloud: bool,
    pub content_negotiation: ContentNegotiationConfig,
    pub security_headers: SecurityHeadersConfig,
    pub allow_registration: bool,
    pub login_page_url: Option<String>,
    pub system_admin_username: String,
    pub system_admin_email: Option<systemprompt_identifiers::Email>,
}

const REDACTED: &str = "<redacted>";

// Why: `database_url` carries the password and `github_token` is a credential;
// a `?config` in any log line must not print either.
impl std::fmt::Debug for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Config")
            .field("instance_id", &self.instance_id)
            .field("metrics_port", &self.metrics_port)
            .field("max_concurrent_streams", &self.max_concurrent_streams)
            .field("sitename", &self.sitename)
            .field("database_type", &self.database_type)
            .field("database_url", &REDACTED)
            .field("database_write_url", &REDACTED)
            .field("github_link", &self.github_link)
            .field("github_token", &REDACTED)
            .field("system_path", &self.system_path)
            .field("services_path", &self.services_path)
            .field("bin_path", &self.bin_path)
            .field("skills_path", &self.skills_path)
            .field("settings_path", &self.settings_path)
            .field("content_config_path", &self.content_config_path)
            .field("geoip_database_path", &self.geoip_database_path)
            .field("web_path", &self.web_path)
            .field("web_config_path", &self.web_config_path)
            .field("web_metadata_path", &self.web_metadata_path)
            .field("host", &self.host)
            .field("port", &self.port)
            .field("api_server_url", &self.api_server_url)
            .field("api_internal_url", &self.api_internal_url)
            .field("api_external_url", &self.api_external_url)
            .field("jwt_issuer", &self.jwt_issuer)
            .field(
                "jwt_access_token_expiration",
                &self.jwt_access_token_expiration,
            )
            .field(
                "jwt_refresh_token_expiration",
                &self.jwt_refresh_token_expiration,
            )
            .field("jwt_audiences", &self.jwt_audiences)
            .field(
                "allowed_resource_audiences",
                &self.allowed_resource_audiences,
            )
            .field("trusted_issuers", &self.trusted_issuers)
            .field("id_jag_ttl_secs", &self.id_jag_ttl_secs)
            .field("signing_key_path", &self.signing_key_path)
            .field("use_https", &self.use_https)
            .field("rate_limits", &self.rate_limits)
            .field("cors_allowed_origins", &self.cors_allowed_origins)
            .field("trusted_proxies", &self.trusted_proxies)
            .field("is_cloud", &self.is_cloud)
            .field("content_negotiation", &self.content_negotiation)
            .field("security_headers", &self.security_headers)
            .field("allow_registration", &self.allow_registration)
            .field("login_page_url", &self.login_page_url)
            .field("system_admin_username", &self.system_admin_username)
            .field("system_admin_email", &self.system_admin_email)
            .finish()
    }
}

impl Config {
    pub fn is_initialized() -> bool {
        CONFIG.get().is_some()
    }

    pub fn get() -> Result<&'static Self, crate::errors::ConfigError> {
        CONFIG
            .get()
            .ok_or(crate::errors::ConfigError::NotInitialized)
    }

    pub fn install(config: Self) -> Result<(), Box<Self>> {
        CONFIG.set(config).map_err(Box::new)
    }

    pub fn logs_path(&self) -> String {
        format!("{}/logs", self.system_path)
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
            "instance_id" => Some(self.instance_id.clone()),
            "max_concurrent_streams" => Some(self.max_concurrent_streams.to_string()),
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
