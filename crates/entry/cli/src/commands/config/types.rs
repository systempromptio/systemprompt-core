use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize)]
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

#[derive(Debug, Clone, Copy, Serialize)]
pub struct TierMultipliersOutput {
    pub admin: f64,
    pub user: f64,
    pub a2a: f64,
    pub mcp: f64,
    pub service: f64,
    pub anon: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct TierEffectiveLimitsOutput {
    pub tier: String,
    pub multiplier: f64,
    pub effective_limits: EffectiveLimitsOutput,
}

#[derive(Debug, Clone, Copy, Serialize)]
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
