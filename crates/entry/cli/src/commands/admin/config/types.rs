use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub use super::config_section::*;
pub use super::rate_limit_types::*;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ConfigOverviewOutput {
    pub profile_name: String,
    pub profile_path: String,
    pub server: ServerOverview,
    pub runtime: RuntimeOverview,
    pub security: SecurityOverview,
    pub paths: PathsOverview,
    pub rate_limits: RateLimitsSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ServerOverview {
    pub host: String,
    pub port: u16,
    pub use_https: bool,
    pub cors_origins_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RuntimeOverview {
    pub environment: String,
    pub log_level: String,
    pub output_format: String,
    pub no_color: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SecurityOverview {
    pub jwt_issuer: String,
    pub access_token_expiry_seconds: i64,
    pub refresh_token_expiry_seconds: i64,
    pub audiences_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PathsOverview {
    pub system: String,
    pub services: String,
    pub bin: String,
    pub web_path: Option<String>,
    pub storage: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
pub struct RateLimitsSummary {
    pub enabled: bool,
    pub burst_multiplier: u64,
    pub tier_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ServerConfigOutput {
    pub host: String,
    pub port: u16,
    pub api_server_url: String,
    pub api_internal_url: String,
    pub api_external_url: String,
    pub use_https: bool,
    pub cors_allowed_origins: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ServerSetOutput {
    pub field: String,
    pub old_value: String,
    pub new_value: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CorsListOutput {
    pub origins: Vec<String>,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CorsModifyOutput {
    pub action: String,
    pub origin: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RuntimeConfigOutput {
    pub environment: String,
    pub log_level: String,
    pub output_format: String,
    pub no_color: bool,
    pub non_interactive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RuntimeSetOutput {
    pub field: String,
    pub old_value: String,
    pub new_value: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SecurityConfigOutput {
    pub jwt_issuer: String,
    pub access_token_expiry_seconds: i64,
    pub refresh_token_expiry_seconds: i64,
    pub audiences: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SecuritySetOutput {
    pub field: String,
    pub old_value: String,
    pub new_value: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PathsConfigOutput {
    pub system: PathInfo,
    pub services: PathInfo,
    pub bin: PathInfo,
    pub web_path: Option<PathInfo>,
    pub storage: Option<PathInfo>,
    pub geoip_database: Option<PathInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PathInfo {
    pub path: String,
    pub exists: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PathsValidateOutput {
    pub valid: bool,
    pub paths: Vec<PathValidation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PathValidation {
    pub name: String,
    pub path: String,
    pub exists: bool,
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ProviderInfo {
    pub name: String,
    pub enabled: bool,
    pub is_default: bool,
    pub model: String,
    pub endpoint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ProviderListOutput {
    pub providers: Vec<ProviderInfo>,
    pub default_provider: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ProviderSetOutput {
    pub provider: String,
    pub action: String,
    pub message: String,
}
