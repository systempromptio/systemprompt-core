use anyhow::Result;
use systemprompt_logging::CliService;
use systemprompt_models::profile::TierMultipliers;
use systemprompt_models::ProfileBootstrap;

use super::helpers::apply_multiplier;
use crate::cli_settings::OutputFormat;
use crate::shared::{render_result, CommandResult};
use crate::CliConfig;

use super::super::types::{CompareOutput, EndpointComparison, ValidateOutput};

pub fn execute_validate(config: &CliConfig) -> Result<()> {
    let profile = ProfileBootstrap::get()?;
    let limits = &profile.rate_limits;

    let mut errors: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();

    if limits.oauth_public_per_second == 0 {
        errors.push("oauth_public_per_second is 0".to_string());
    }
    if limits.oauth_auth_per_second == 0 {
        errors.push("oauth_auth_per_second is 0".to_string());
    }
    if limits.contexts_per_second == 0 {
        errors.push("contexts_per_second is 0".to_string());
    }
    if limits.tasks_per_second == 0 {
        errors.push("tasks_per_second is 0".to_string());
    }
    if limits.artifacts_per_second == 0 {
        errors.push("artifacts_per_second is 0".to_string());
    }
    if limits.agents_per_second == 0 {
        errors.push("agents_per_second is 0".to_string());
    }
    if limits.mcp_per_second == 0 {
        errors.push("mcp_per_second is 0".to_string());
    }
    if limits.stream_per_second == 0 {
        errors.push("stream_per_second is 0".to_string());
    }
    if limits.content_per_second == 0 {
        errors.push("content_per_second is 0".to_string());
    }

    let tiers = &limits.tier_multipliers;
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

    if tiers.admin <= 0.0 {
        errors.push("admin multiplier must be positive".to_string());
    }
    if tiers.user <= 0.0 {
        errors.push("user multiplier must be positive".to_string());
    }
    if tiers.anon <= 0.0 {
        errors.push("anon multiplier must be positive".to_string());
    }
    if tiers.a2a <= 0.0 {
        errors.push("a2a multiplier must be positive".to_string());
    }
    if tiers.mcp <= 0.0 {
        errors.push("mcp multiplier must be positive".to_string());
    }
    if tiers.service <= 0.0 {
        errors.push("service multiplier must be positive".to_string());
    }

    if limits.burst_multiplier == 0 {
        errors.push("burst_multiplier is 0".to_string());
    }
    if limits.burst_multiplier > 10 {
        warnings.push(format!(
            "burst_multiplier {} exceeds recommended maximum of 10",
            limits.burst_multiplier
        ));
    }

    if limits.disabled {
        warnings.push("Rate limiting is currently DISABLED".to_string());
    }

    let valid = errors.is_empty();
    let output = ValidateOutput {
        valid,
        errors,
        warnings,
    };

    render_result(&CommandResult::card(output).with_title("Rate Limits Validation"));

    if config.output_format() == OutputFormat::Table {
        if valid {
            CliService::success("Configuration is valid");
        } else {
            CliService::error("Configuration has errors");
        }
    }

    Ok(())
}

pub fn execute_compare(config: &CliConfig) -> Result<()> {
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

    render_result(&CommandResult::table(output).with_title("Rate Limits Comparison"));

    if config.output_format() == OutputFormat::Table && limits.disabled {
        CliService::warning("Rate limiting is currently DISABLED");
    }

    Ok(())
}

fn create_comparison(name: &str, base: u64, tiers: &TierMultipliers) -> EndpointComparison {
    EndpointComparison {
        endpoint: name.to_string(),
        admin: apply_multiplier(base, tiers.admin),
        user: apply_multiplier(base, tiers.user),
        a2a: apply_multiplier(base, tiers.a2a),
        mcp: apply_multiplier(base, tiers.mcp),
        service: apply_multiplier(base, tiers.service),
        anon: apply_multiplier(base, tiers.anon),
    }
}
