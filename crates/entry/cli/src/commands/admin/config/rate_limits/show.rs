use anyhow::Result;
use systemprompt_config::ProfileBootstrap;
use systemprompt_logging::CliService;
use systemprompt_models::profile::{RateLimitsConfig, TierMultipliers};

use super::TierArgs;
use super::helpers::{apply_multiplier, get_tier_multiplier};
use crate::CliConfig;
use crate::cli_settings::OutputFormat;
use crate::shared::{CommandOutput, render_result};

use super::super::types::{
    BaseRateRow, EffectiveLimitRow, EffectiveLimitsOutput, EndpointRateLimit, RateLimitsDocsOutput,
    RateLimitsOutput, TierEffectiveLimitsOutput, TierMultiplierRow, TierMultipliersOutput,
};

pub(super) fn execute_show(config: &CliConfig) -> Result<()> {
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

    render_result(&CommandOutput::card_value("Rate Limits", &output), config);

    if config.output_format() == OutputFormat::Table && limits.disabled {
        CliService::warning("Rate limiting is currently DISABLED");
    }

    Ok(())
}

pub(super) fn execute_tier(args: TierArgs, config: &CliConfig) -> Result<()> {
    let profile = ProfileBootstrap::get()?;
    let limits = &profile.rate_limits;

    let multiplier = get_tier_multiplier(&limits.tier_multipliers, &args.tier)?;

    let output = TierEffectiveLimitsOutput {
        tier: args.tier,
        multiplier,
        effective_limits: EffectiveLimitsOutput {
            limits: vec![
                EndpointRateLimit {
                    endpoint: "oauth_public".to_owned(),
                    per_second: apply_multiplier(limits.oauth_public_per_second, multiplier),
                },
                EndpointRateLimit {
                    endpoint: "oauth_auth".to_owned(),
                    per_second: apply_multiplier(limits.oauth_auth_per_second, multiplier),
                },
                EndpointRateLimit {
                    endpoint: "contexts".to_owned(),
                    per_second: apply_multiplier(limits.contexts_per_second, multiplier),
                },
                EndpointRateLimit {
                    endpoint: "tasks".to_owned(),
                    per_second: apply_multiplier(limits.tasks_per_second, multiplier),
                },
                EndpointRateLimit {
                    endpoint: "artifacts".to_owned(),
                    per_second: apply_multiplier(limits.artifacts_per_second, multiplier),
                },
                EndpointRateLimit {
                    endpoint: "agent_registry".to_owned(),
                    per_second: apply_multiplier(limits.agent_registry_per_second, multiplier),
                },
                EndpointRateLimit {
                    endpoint: "agents".to_owned(),
                    per_second: apply_multiplier(limits.agents_per_second, multiplier),
                },
                EndpointRateLimit {
                    endpoint: "mcp_registry".to_owned(),
                    per_second: apply_multiplier(limits.mcp_registry_per_second, multiplier),
                },
                EndpointRateLimit {
                    endpoint: "mcp".to_owned(),
                    per_second: apply_multiplier(limits.mcp_per_second, multiplier),
                },
                EndpointRateLimit {
                    endpoint: "stream".to_owned(),
                    per_second: apply_multiplier(limits.stream_per_second, multiplier),
                },
                EndpointRateLimit {
                    endpoint: "content".to_owned(),
                    per_second: apply_multiplier(limits.content_per_second, multiplier),
                },
            ],
        },
    };

    render_result(
        &CommandOutput::card_value("Tier Effective Limits", &output),
        config,
    );

    if config.output_format() == OutputFormat::Table && limits.disabled {
        CliService::warning("Rate limiting is currently DISABLED");
    }

    Ok(())
}

pub(super) fn execute_docs(config: &CliConfig) -> Result<()> {
    let profile = ProfileBootstrap::get()?;
    let limits = &profile.rate_limits;

    let output = RateLimitsDocsOutput {
        base_rates: base_rate_rows(limits),
        tier_multipliers: tier_multiplier_rows(&limits.tier_multipliers),
        effective_limits: effective_limit_rows(limits),
        burst_multiplier: limits.burst_multiplier,
        disabled: limits.disabled,
    };

    render_result(
        &CommandOutput::card_value("Rate Limits Documentation", &output),
        config,
    );

    if config.output_format() == OutputFormat::Table && limits.disabled {
        CliService::warning("Rate limiting is currently DISABLED");
    }

    Ok(())
}

fn base_rate_rows(limits: &RateLimitsConfig) -> Vec<BaseRateRow> {
    [
        ("OAuth Public", limits.oauth_public_per_second),
        ("OAuth Auth", limits.oauth_auth_per_second),
        ("Contexts", limits.contexts_per_second),
        ("Tasks", limits.tasks_per_second),
        ("Artifacts", limits.artifacts_per_second),
        ("Agent Registry", limits.agent_registry_per_second),
        ("Agents", limits.agents_per_second),
        ("MCP Registry", limits.mcp_registry_per_second),
        ("MCP", limits.mcp_per_second),
        ("Stream (SSE)", limits.stream_per_second),
        ("Content", limits.content_per_second),
    ]
    .into_iter()
    .map(|(endpoint, rate_per_second)| BaseRateRow {
        endpoint: endpoint.to_owned(),
        rate_per_second,
    })
    .collect()
}

fn tier_multiplier_rows(tiers: &TierMultipliers) -> Vec<TierMultiplierRow> {
    [
        ("Admin", tiers.admin),
        ("User", tiers.user),
        ("A2A", tiers.a2a),
        ("MCP", tiers.mcp),
        ("Service", tiers.service),
        ("Anonymous", tiers.anon),
    ]
    .into_iter()
    .map(|(tier, multiplier)| TierMultiplierRow {
        tier: tier.to_owned(),
        multiplier,
    })
    .collect()
}

fn effective_limit_rows(limits: &RateLimitsConfig) -> Vec<EffectiveLimitRow> {
    let tiers = &limits.tier_multipliers;
    [
        ("Contexts", limits.contexts_per_second),
        ("Tasks", limits.tasks_per_second),
        ("Agents", limits.agents_per_second),
        ("Stream (SSE)", limits.stream_per_second),
        ("MCP", limits.mcp_per_second),
    ]
    .into_iter()
    .map(|(endpoint, base)| EffectiveLimitRow {
        endpoint: endpoint.to_owned(),
        admin: apply_multiplier(base, tiers.admin),
        user: apply_multiplier(base, tiers.user),
        anon: apply_multiplier(base, tiers.anon),
    })
    .collect()
}
