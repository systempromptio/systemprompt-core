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
