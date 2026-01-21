use anyhow::{bail, Result};
use systemprompt_logging::CliService;
use systemprompt_models::ProfileBootstrap;

use super::helpers::{
    get_endpoint_rate, get_tier_multiplier, load_profile_for_edit, save_profile, set_endpoint_rate,
    set_tier_multiplier,
};
use super::SetArgs;
use crate::cli_settings::OutputFormat;
use crate::shared::{render_result, CommandResult};
use crate::CliConfig;

use super::super::types::{RateLimitStatusOutput, SetRateLimitOutput};

pub fn execute_set(args: &SetArgs, config: &CliConfig) -> Result<()> {
    if args.endpoint.is_some() && args.rate.is_none() {
        bail!("--rate is required when --endpoint is specified");
    }
    if args.rate.is_some() && args.endpoint.is_none() {
        bail!("--endpoint is required when --rate is specified");
    }
    if args.tier.is_some() && args.multiplier.is_none() {
        bail!("--multiplier is required when --tier is specified");
    }
    if args.multiplier.is_some() && args.tier.is_none() {
        bail!("--tier is required when --multiplier is specified");
    }
    if args.endpoint.is_none() && args.tier.is_none() && args.burst.is_none() {
        bail!("Must specify one of: --endpoint with --rate, --tier with --multiplier, or --burst");
    }

    let profile_path = ProfileBootstrap::get_path()?;
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
    } else if let (Some(tier), Some(multiplier)) = (&args.tier, args.multiplier) {
        let old_value = get_tier_multiplier(&limits.tier_multipliers, tier)?;
        set_tier_multiplier(&mut limits.tier_multipliers, tier, multiplier)?;
        SetRateLimitOutput {
            field: format!("tier_multipliers.{}", tier),
            old_value: format!("{:.1}", old_value),
            new_value: format!("{:.1}", multiplier),
            message: format!(
                "Updated {} tier multiplier: {:.1}x -> {:.1}x",
                tier, old_value, multiplier
            ),
        }
    } else if let Some(burst) = args.burst {
        let old_value = limits.burst_multiplier;
        limits.burst_multiplier = burst;
        SetRateLimitOutput {
            field: "burst_multiplier".to_string(),
            old_value: old_value.to_string(),
            new_value: burst.to_string(),
            message: format!("Updated burst multiplier: {}x -> {}x", old_value, burst),
        }
    } else {
        bail!("Invalid arguments");
    };

    save_profile(&profile, profile_path)?;
    render_result(&CommandResult::text(output).with_title("Rate Limit Updated"));

    if config.output_format() == OutputFormat::Table {
        CliService::warning("Restart services for changes to take effect");
    }

    Ok(())
}

pub fn execute_enable(config: &CliConfig) -> Result<()> {
    let profile_path = ProfileBootstrap::get_path()?;
    let mut profile = load_profile_for_edit(profile_path)?;

    if !profile.rate_limits.disabled {
        let output = RateLimitStatusOutput {
            enabled: true,
            message: "Rate limiting is already enabled".to_string(),
        };
        render_result(&CommandResult::text(output).with_title("Rate Limit Status"));
        return Ok(());
    }

    profile.rate_limits.disabled = false;
    save_profile(&profile, profile_path)?;

    let output = RateLimitStatusOutput {
        enabled: true,
        message: "Rate limiting enabled".to_string(),
    };
    render_result(&CommandResult::text(output).with_title("Rate Limit Status"));

    if config.output_format() == OutputFormat::Table {
        CliService::warning("Restart services for changes to take effect");
    }

    Ok(())
}

pub fn execute_disable(config: &CliConfig) -> Result<()> {
    let profile_path = ProfileBootstrap::get_path()?;
    let mut profile = load_profile_for_edit(profile_path)?;

    if profile.rate_limits.disabled {
        let output = RateLimitStatusOutput {
            enabled: false,
            message: "Rate limiting is already disabled".to_string(),
        };
        render_result(&CommandResult::text(output).with_title("Rate Limit Status"));
        return Ok(());
    }

    profile.rate_limits.disabled = true;
    save_profile(&profile, profile_path)?;

    let output = RateLimitStatusOutput {
        enabled: false,
        message: "Rate limiting disabled".to_string(),
    };
    render_result(&CommandResult::text(output).with_title("Rate Limit Status"));

    if config.output_format() == OutputFormat::Table {
        CliService::warning("Restart services for changes to take effect");
    }

    Ok(())
}
