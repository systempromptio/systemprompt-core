use anyhow::Result;
use systemprompt_logging::CliService;
use systemprompt_models::ProfileBootstrap;

use super::helpers::{apply_multiplier, get_tier_multiplier};
use super::TierArgs;
use crate::cli_settings::OutputFormat;
use crate::shared::{render_result, CommandResult};
use crate::CliConfig;

use super::super::types::{
    BaseRateRow, EffectiveLimitRow, EffectiveLimitsOutput, RateLimitsDocsOutput, RateLimitsOutput,
    TierEffectiveLimitsOutput, TierMultiplierRow, TierMultipliersOutput,
};

pub fn execute_show(config: &CliConfig) -> Result<()> {
    let profile = ProfileBootstrap::get()?;
    let limits = &profile.rate_limits;

    let output = RateLimitsOutput {
        disabled: limits.disabled,
        oauth_public_per_second: limits.oauth_public_per_second,
        oauth_auth_per_second: limits.oauth_auth_per_second,
        contexts_per_second: limits.contexts_per_second,
        tasks_per_second: limits.tasks_per_second,
        artifacts_per_second: limits.artifacts_per_second,
        agent_registry_per_second: limits.agent_registry_per_second,
        agents_per_second: limits.agents_per_second,
        mcp_registry_per_second: limits.mcp_registry_per_second,
        mcp_per_second: limits.mcp_per_second,
        stream_per_second: limits.stream_per_second,
        content_per_second: limits.content_per_second,
        burst_multiplier: limits.burst_multiplier,
        tier_multipliers: TierMultipliersOutput {
            admin: limits.tier_multipliers.admin,
            user: limits.tier_multipliers.user,
            a2a: limits.tier_multipliers.a2a,
            mcp: limits.tier_multipliers.mcp,
            service: limits.tier_multipliers.service,
            anon: limits.tier_multipliers.anon,
        },
    };

    render_result(&CommandResult::card(output).with_title("Rate Limits"));

    if config.output_format() == OutputFormat::Table && limits.disabled {
        CliService::warning("Rate limiting is currently DISABLED");
    }

    Ok(())
}

pub fn execute_tier(args: TierArgs, config: &CliConfig) -> Result<()> {
    let profile = ProfileBootstrap::get()?;
    let limits = &profile.rate_limits;

    let multiplier = get_tier_multiplier(&limits.tier_multipliers, &args.tier)?;

    let output = TierEffectiveLimitsOutput {
        tier: args.tier,
        multiplier,
        effective_limits: EffectiveLimitsOutput {
            oauth_public_per_second: apply_multiplier(limits.oauth_public_per_second, multiplier),
            oauth_auth_per_second: apply_multiplier(limits.oauth_auth_per_second, multiplier),
            contexts_per_second: apply_multiplier(limits.contexts_per_second, multiplier),
            tasks_per_second: apply_multiplier(limits.tasks_per_second, multiplier),
            artifacts_per_second: apply_multiplier(limits.artifacts_per_second, multiplier),
            agent_registry_per_second: apply_multiplier(
                limits.agent_registry_per_second,
                multiplier,
            ),
            agents_per_second: apply_multiplier(limits.agents_per_second, multiplier),
            mcp_registry_per_second: apply_multiplier(limits.mcp_registry_per_second, multiplier),
            mcp_per_second: apply_multiplier(limits.mcp_per_second, multiplier),
            stream_per_second: apply_multiplier(limits.stream_per_second, multiplier),
            content_per_second: apply_multiplier(limits.content_per_second, multiplier),
        },
    };

    render_result(&CommandResult::card(output).with_title("Tier Effective Limits"));

    if config.output_format() == OutputFormat::Table && limits.disabled {
        CliService::warning("Rate limiting is currently DISABLED");
    }

    Ok(())
}

pub fn execute_docs(config: &CliConfig) -> Result<()> {
    let profile = ProfileBootstrap::get()?;
    let limits = &profile.rate_limits;

    let admin_mult = limits.tier_multipliers.admin;
    let user_mult = limits.tier_multipliers.user;
    let anon_mult = limits.tier_multipliers.anon;

    let output = RateLimitsDocsOutput {
        base_rates: vec![
            BaseRateRow {
                endpoint: "OAuth Public".to_string(),
                rate_per_second: limits.oauth_public_per_second,
            },
            BaseRateRow {
                endpoint: "OAuth Auth".to_string(),
                rate_per_second: limits.oauth_auth_per_second,
            },
            BaseRateRow {
                endpoint: "Contexts".to_string(),
                rate_per_second: limits.contexts_per_second,
            },
            BaseRateRow {
                endpoint: "Tasks".to_string(),
                rate_per_second: limits.tasks_per_second,
            },
            BaseRateRow {
                endpoint: "Artifacts".to_string(),
                rate_per_second: limits.artifacts_per_second,
            },
            BaseRateRow {
                endpoint: "Agent Registry".to_string(),
                rate_per_second: limits.agent_registry_per_second,
            },
            BaseRateRow {
                endpoint: "Agents".to_string(),
                rate_per_second: limits.agents_per_second,
            },
            BaseRateRow {
                endpoint: "MCP Registry".to_string(),
                rate_per_second: limits.mcp_registry_per_second,
            },
            BaseRateRow {
                endpoint: "MCP".to_string(),
                rate_per_second: limits.mcp_per_second,
            },
            BaseRateRow {
                endpoint: "Stream (SSE)".to_string(),
                rate_per_second: limits.stream_per_second,
            },
            BaseRateRow {
                endpoint: "Content".to_string(),
                rate_per_second: limits.content_per_second,
            },
        ],
        tier_multipliers: vec![
            TierMultiplierRow {
                tier: "Admin".to_string(),
                multiplier: limits.tier_multipliers.admin,
            },
            TierMultiplierRow {
                tier: "User".to_string(),
                multiplier: limits.tier_multipliers.user,
            },
            TierMultiplierRow {
                tier: "A2A".to_string(),
                multiplier: limits.tier_multipliers.a2a,
            },
            TierMultiplierRow {
                tier: "MCP".to_string(),
                multiplier: limits.tier_multipliers.mcp,
            },
            TierMultiplierRow {
                tier: "Service".to_string(),
                multiplier: limits.tier_multipliers.service,
            },
            TierMultiplierRow {
                tier: "Anonymous".to_string(),
                multiplier: limits.tier_multipliers.anon,
            },
        ],
        effective_limits: vec![
            EffectiveLimitRow {
                endpoint: "Contexts".to_string(),
                admin: apply_multiplier(limits.contexts_per_second, admin_mult),
                user: apply_multiplier(limits.contexts_per_second, user_mult),
                anon: apply_multiplier(limits.contexts_per_second, anon_mult),
            },
            EffectiveLimitRow {
                endpoint: "Tasks".to_string(),
                admin: apply_multiplier(limits.tasks_per_second, admin_mult),
                user: apply_multiplier(limits.tasks_per_second, user_mult),
                anon: apply_multiplier(limits.tasks_per_second, anon_mult),
            },
            EffectiveLimitRow {
                endpoint: "Agents".to_string(),
                admin: apply_multiplier(limits.agents_per_second, admin_mult),
                user: apply_multiplier(limits.agents_per_second, user_mult),
                anon: apply_multiplier(limits.agents_per_second, anon_mult),
            },
            EffectiveLimitRow {
                endpoint: "Stream (SSE)".to_string(),
                admin: apply_multiplier(limits.stream_per_second, admin_mult),
                user: apply_multiplier(limits.stream_per_second, user_mult),
                anon: apply_multiplier(limits.stream_per_second, anon_mult),
            },
            EffectiveLimitRow {
                endpoint: "MCP".to_string(),
                admin: apply_multiplier(limits.mcp_per_second, admin_mult),
                user: apply_multiplier(limits.mcp_per_second, user_mult),
                anon: apply_multiplier(limits.mcp_per_second, anon_mult),
            },
        ],
        burst_multiplier: limits.burst_multiplier,
        disabled: limits.disabled,
    };

    render_result(&CommandResult::table(output).with_title("Rate Limits Documentation"));

    if config.output_format() == OutputFormat::Table && limits.disabled {
        CliService::warning("Rate limiting is currently DISABLED");
    }

    Ok(())
}
