//! Rate-limit config validation checks.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use systemprompt_config::ProfileBootstrap;
use systemprompt_logging::CliService;
use systemprompt_models::profile::{RateLimitsConfig, TierMultipliers};

use super::helpers::apply_multiplier;
use crate::CliConfig;
use crate::cli_settings::OutputFormat;
use crate::shared::{CommandOutput, render_result};

use super::super::types::{CompareOutput, EndpointComparison, ValidateOutput};

pub(super) fn execute_validate(config: &CliConfig) -> Result<()> {
    let profile = ProfileBootstrap::get()?;
    let limits = &profile.rate_limits;

    let mut errors: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();

    check_endpoint_rates(limits, &mut errors);
    check_tier_multipliers(&limits.tier_multipliers, &mut errors, &mut warnings);
    check_burst_and_disabled(limits, &mut errors, &mut warnings);

    let valid = errors.is_empty();
    let output = ValidateOutput {
        valid,
        errors,
        warnings,
    };

    render_result(
        &CommandOutput::card_value("Rate Limits Validation", &output),
        config,
    );

    if config.output_format() == OutputFormat::Table {
        if valid {
            CliService::success("Configuration is valid");
        } else {
            CliService::error("Configuration has errors");
        }
    }

    Ok(())
}

fn check_endpoint_rates(limits: &RateLimitsConfig, errors: &mut Vec<String>) {
    let rates = [
        ("oauth_public_per_second", limits.oauth_public_per_second),
        ("oauth_auth_per_second", limits.oauth_auth_per_second),
        ("contexts_per_second", limits.contexts_per_second),
        ("tasks_per_second", limits.tasks_per_second),
        ("artifacts_per_second", limits.artifacts_per_second),
        ("agents_per_second", limits.agents_per_second),
        ("mcp_per_second", limits.mcp_per_second),
        ("stream_per_second", limits.stream_per_second),
        ("content_per_second", limits.content_per_second),
    ];

    for (name, value) in rates {
        if value == 0 {
            errors.push(format!("{name} is 0"));
        }
    }
}

fn check_tier_multipliers(
    tiers: &TierMultipliers,
    errors: &mut Vec<String>,
    warnings: &mut Vec<String>,
) {
    if tiers.anon >= tiers.user {
        warnings.push(format!(
            "anon multiplier ({:.1}) >= user multiplier ({:.1})",
            tiers.anon, tiers.user
        ));
    }
    if tiers.user >= tiers.admin {
        warnings.push(format!(
            "user multiplier ({:.1}) >= admin multiplier ({:.1})",
            tiers.user, tiers.admin
        ));
    }

    let multipliers = [
        ("admin", tiers.admin),
        ("user", tiers.user),
        ("anon", tiers.anon),
        ("a2a", tiers.a2a),
        ("mcp", tiers.mcp),
        ("service", tiers.service),
    ];

    for (name, value) in multipliers {
        if value <= 0.0 {
            errors.push(format!("{name} multiplier must be positive"));
        }
    }
}

fn check_burst_and_disabled(
    limits: &RateLimitsConfig,
    errors: &mut Vec<String>,
    warnings: &mut Vec<String>,
) {
    if limits.burst_multiplier == 0 {
        errors.push("burst_multiplier is 0".to_owned());
    }
    if limits.burst_multiplier > 10 {
        warnings.push(format!(
            "burst_multiplier {} exceeds recommended maximum of 10",
            limits.burst_multiplier
        ));
    }
    if limits.disabled {
        warnings.push("Rate limiting is currently DISABLED".to_owned());
    }
}

pub(super) fn execute_compare(config: &CliConfig) -> Result<()> {
    let profile = ProfileBootstrap::get()?;
    let limits = &profile.rate_limits;
    let tiers = &limits.tier_multipliers;

    let endpoints = vec![
        create_comparison("OAuth Public", limits.oauth_public_per_second, tiers),
        create_comparison("OAuth Auth", limits.oauth_auth_per_second, tiers),
        create_comparison("Contexts", limits.contexts_per_second, tiers),
        create_comparison("Tasks", limits.tasks_per_second, tiers),
        create_comparison("Artifacts", limits.artifacts_per_second, tiers),
        create_comparison("Agent Registry", limits.agent_registry_per_second, tiers),
        create_comparison("Agents", limits.agents_per_second, tiers),
        create_comparison("MCP Registry", limits.mcp_registry_per_second, tiers),
        create_comparison("MCP", limits.mcp_per_second, tiers),
        create_comparison("Stream (SSE)", limits.stream_per_second, tiers),
        create_comparison("Content", limits.content_per_second, tiers),
    ];

    let output = CompareOutput { endpoints };

    render_result(
        &CommandOutput::table_of(
            vec!["endpoint", "admin", "user", "a2a", "mcp", "service", "anon"],
            &output.endpoints,
        )
        .with_title("Rate Limits Comparison"),
        config,
    );

    if config.output_format() == OutputFormat::Table && limits.disabled {
        CliService::warning("Rate limiting is currently DISABLED");
    }

    Ok(())
}

fn create_comparison(name: &str, base: u64, tiers: &TierMultipliers) -> EndpointComparison {
    EndpointComparison {
        endpoint: name.to_owned(),
        admin: apply_multiplier(base, tiers.admin),
        user: apply_multiplier(base, tiers.user),
        a2a: apply_multiplier(base, tiers.a2a),
        mcp: apply_multiplier(base, tiers.mcp),
        service: apply_multiplier(base, tiers.service),
        anon: apply_multiplier(base, tiers.anon),
    }
}
