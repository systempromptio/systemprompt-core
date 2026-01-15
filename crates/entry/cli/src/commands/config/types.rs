use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
pub struct RateLimitsOutput {
    pub disabled: bool,
    pub oauth_public_per_second: u64,
    pub oauth_auth_per_second: u64,
    pub contexts_per_second: u64,
    pub tasks_per_second: u64,
    pub artifacts_per_second: u64,
    pub agent_registry_per_second: u64,
    pub agents_per_second: u64,
    pub mcp_registry_per_second: u64,
    pub mcp_per_second: u64,
    pub stream_per_second: u64,
    pub content_per_second: u64,
    pub burst_multiplier: u64,
    pub tier_multipliers: TierMultipliersOutput,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
pub struct TierMultipliersOutput {
    pub admin: f64,
    pub user: f64,
    pub a2a: f64,
    pub mcp: f64,
    pub service: f64,
    pub anon: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TierEffectiveLimitsOutput {
    pub tier: String,
    pub multiplier: f64,
    pub effective_limits: EffectiveLimitsOutput,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[allow(clippy::struct_field_names)]
pub struct EffectiveLimitsOutput {
    pub oauth_public_per_second: u64,
    pub oauth_auth_per_second: u64,
    pub contexts_per_second: u64,
    pub tasks_per_second: u64,
    pub artifacts_per_second: u64,
    pub agent_registry_per_second: u64,
    pub agents_per_second: u64,
    pub mcp_registry_per_second: u64,
    pub mcp_per_second: u64,
    pub stream_per_second: u64,
    pub content_per_second: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RateLimitsDocsOutput {
    pub base_rates: Vec<BaseRateRow>,
    pub tier_multipliers: Vec<TierMultiplierRow>,
    pub effective_limits: Vec<EffectiveLimitRow>,
    pub burst_multiplier: u64,
    pub disabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BaseRateRow {
    pub endpoint: String,
    pub rate_per_second: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TierMultiplierRow {
    pub tier: String,
    pub multiplier: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EffectiveLimitRow {
    pub endpoint: String,
    pub admin: u64,
    pub user: u64,
    pub anon: u64,
}


#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SetRateLimitOutput {
    pub field: String,
    pub old_value: String,
    pub new_value: String,
    pub message: String,
}


#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RateLimitStatusOutput {
    pub enabled: bool,
    pub message: String,
}


#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ValidateOutput {
    pub valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}


#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CompareOutput {
    pub endpoints: Vec<EndpointComparison>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EndpointComparison {
    pub endpoint: String,
    pub admin: u64,
    pub user: u64,
    pub a2a: u64,
    pub mcp: u64,
    pub service: u64,
    pub anon: u64,
}


#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ResetOutput {
    pub reset_type: String,
    pub changes: Vec<ResetChange>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ResetChange {
    pub field: String,
    pub old_value: String,
    pub new_value: String,
}


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
pub struct PresetListOutput {
    pub presets: Vec<PresetInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PresetInfo {
    pub name: String,
    pub description: String,
    pub builtin: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PresetShowOutput {
    pub name: String,
    pub description: String,
    pub config: RateLimitsOutput,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PresetApplyOutput {
    pub preset: String,
    pub changes: Vec<ResetChange>,
    pub message: String,
}


#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExportOutput {
    pub format: String,
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ImportOutput {
    pub path: String,
    pub changes: Vec<ResetChange>,
    pub message: String,
}


#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DiffOutput {
    pub source: String,
    pub differences: Vec<DiffEntry>,
    pub identical: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DiffEntry {
    pub field: String,
    pub current: String,
    pub other: String,
}
