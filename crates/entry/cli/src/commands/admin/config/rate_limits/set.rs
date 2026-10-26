//! `admin config rate-limits set` command.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Result, bail};
use systemprompt_logging::CliService;

use super::super::profile_io::profile_path;
use super::SetArgs;
use super::profile_edit::{
    get_endpoint_rate, load_profile_for_edit, save_profile, set_endpoint_rate,
};
use crate::cli_settings::OutputFormat;
use crate::context::CommandContext;
use crate::shared::{CommandOutput, render_result};

use super::super::types::{RateLimitStatusOutput, SetRateLimitOutput};

pub(super) fn execute_set(args: &SetArgs, ctx: &CommandContext) -> Result<()> {
    let config = &ctx.cli;
    if args.endpoint.is_some() && args.rate.is_none() {
        bail!("--rate is required when --endpoint is specified");
    }
    if args.rate.is_some() && args.endpoint.is_none() {
        bail!("--endpoint is required when --rate is specified");
    }
    if args.endpoint.is_none() && args.burst.is_none() {
        bail!("Must specify one of: --endpoint with --rate, or --burst");
    }

    let profile_path = profile_path(ctx)?;
    let mut profile = load_profile_for_edit(profile_path)?;
    let limits = &mut profile.rate_limits;

    let output = if let (Some(endpoint), Some(rate)) = (&args.endpoint, args.rate) {
        let old_value = get_endpoint_rate(limits, endpoint)?;
        set_endpoint_rate(limits, endpoint, rate)?;
        SetRateLimitOutput {
            field: format!("{}_per_second", endpoint),
            old_value: old_value.to_string(),
            new_value: rate.to_string(),
            message: format!("Updated {} rate: {} -> {}/s", endpoint, old_value, rate),
        }
    } else if let Some(burst) = args.burst {
        let old_value = limits.burst_multiplier;
        limits.burst_multiplier = burst;
        SetRateLimitOutput {
            field: "burst_multiplier".to_owned(),
            old_value: old_value.to_string(),
            new_value: burst.to_string(),
            message: format!("Updated burst multiplier: {}x -> {}x", old_value, burst),
        }
    } else {
        bail!("Invalid arguments");
    };

    save_profile(&profile, profile_path)?;
    render_result(
        &CommandOutput::card_value("Rate Limit Updated", &output),
        config,
    );

    if config.output_format() == OutputFormat::Table {
        CliService::warning("Restart services for changes to take effect");
    }

    Ok(())
}

pub(super) fn execute_enable(ctx: &CommandContext) -> Result<()> {
    let config = &ctx.cli;
    let profile_path = profile_path(ctx)?;
    let mut profile = load_profile_for_edit(profile_path)?;

    if !profile.rate_limits.disabled {
        let output = RateLimitStatusOutput {
            enabled: true,
            message: "Rate limiting is already enabled".to_owned(),
        };
        render_result(
            &CommandOutput::card_value("Rate Limit Status", &output),
            config,
        );
        return Ok(());
    }

    profile.rate_limits.disabled = false;
    save_profile(&profile, profile_path)?;

    let output = RateLimitStatusOutput {
        enabled: true,
        message: "Rate limiting enabled".to_owned(),
    };
    render_result(
        &CommandOutput::card_value("Rate Limit Status", &output),
        config,
    );

    if config.output_format() == OutputFormat::Table {
        CliService::warning("Restart services for changes to take effect");
    }

    Ok(())
}

pub(super) fn execute_disable(ctx: &CommandContext) -> Result<()> {
    let config = &ctx.cli;
    let profile_path = profile_path(ctx)?;
    let mut profile = load_profile_for_edit(profile_path)?;

    if profile.rate_limits.disabled {
        let output = RateLimitStatusOutput {
            enabled: false,
            message: "Rate limiting is already disabled".to_owned(),
        };
        render_result(
            &CommandOutput::card_value("Rate Limit Status", &output),
            config,
        );
        return Ok(());
    }

    profile.rate_limits.disabled = true;
    save_profile(&profile, profile_path)?;

    let output = RateLimitStatusOutput {
        enabled: false,
        message: "Rate limiting disabled".to_owned(),
    };
    render_result(
        &CommandOutput::card_value("Rate Limit Status", &output),
        config,
    );

    if config.output_format() == OutputFormat::Table {
        CliService::warning("Restart services for changes to take effect");
    }

    Ok(())
}
