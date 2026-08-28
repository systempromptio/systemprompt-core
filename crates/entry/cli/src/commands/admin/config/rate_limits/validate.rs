//! Rate-limit config validation checks.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use systemprompt_config::ProfileBootstrap;
use systemprompt_logging::CliService;
use systemprompt_models::profile::RateLimitsConfig;

use crate::CliConfig;
use crate::cli_settings::OutputFormat;
use crate::shared::{CommandOutput, render_result};

use super::super::types::ValidateOutput;

pub(super) fn execute_validate(config: &CliConfig) -> Result<()> {
    let profile = ProfileBootstrap::get()?;
    let limits = &profile.rate_limits;

    let mut errors: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();

    check_endpoint_rates(limits, &mut errors);
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
