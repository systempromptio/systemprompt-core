use anyhow::Result;
use systemprompt_logging::CliService;
use systemprompt_models::profile::RateLimitsConfig;
use systemprompt_models::ProfileBootstrap;

use super::helpers::{
    collect_endpoint_changes, collect_tier_changes, get_endpoint_rate, get_tier_multiplier,
    load_profile_for_edit, save_profile, set_endpoint_rate, set_tier_multiplier,
};
use super::ResetArgs;
use crate::cli_settings::OutputFormat;
use crate::interactive::require_confirmation;
use crate::shared::{render_result, CommandResult};
use crate::CliConfig;

use super::super::types::{ResetChange, ResetOutput};

pub fn execute_reset(args: &ResetArgs, config: &CliConfig) -> Result<()> {

    let profile_path = ProfileBootstrap::get_path()?;
    let mut profile = load_profile_for_edit(profile_path)?;
    let limits = &mut profile.rate_limits;
    let defaults = RateLimitsConfig::default();

    let mut changes: Vec<ResetChange> = Vec::new();
    let reset_type: String;

    if let Some(endpoint) = &args.endpoint {
        reset_type = format!("endpoint:{}", endpoint);
        let old_value = get_endpoint_rate(limits, endpoint)?;
        let new_value = get_endpoint_rate(&defaults, endpoint)?;
        if old_value != new_value {
            changes.push(ResetChange {
                field: format!("{}_per_second", endpoint),
                old_value: old_value.to_string(),
                new_value: new_value.to_string(),
            });
            if !args.dry_run {
                set_endpoint_rate(limits, endpoint, new_value)?;
            }
        }
    } else if let Some(tier) = &args.tier {
        reset_type = format!("tier:{}", tier);
        let old_value = get_tier_multiplier(&limits.tier_multipliers, tier)?;
        let new_value = get_tier_multiplier(&defaults.tier_multipliers, tier)?;
        if (old_value - new_value).abs() > f64::EPSILON {
            changes.push(ResetChange {
                field: format!("tier_multipliers.{}", tier),
                old_value: format!("{:.1}", old_value),
                new_value: format!("{:.1}", new_value),
            });
            if !args.dry_run {
                set_tier_multiplier(&mut limits.tier_multipliers, tier, new_value)?;
            }
        }
    } else {
        reset_type = "all".to_string();
        collect_endpoint_changes(limits, &defaults, &mut changes);
        collect_tier_changes(
            &limits.tier_multipliers,
            &defaults.tier_multipliers,
            &mut changes,
        );

        if limits.burst_multiplier != defaults.burst_multiplier {
            changes.push(ResetChange {
                field: "burst_multiplier".to_string(),
                old_value: limits.burst_multiplier.to_string(),
                new_value: defaults.burst_multiplier.to_string(),
            });
        }
        if limits.disabled != defaults.disabled {
            changes.push(ResetChange {
                field: "disabled".to_string(),
                old_value: limits.disabled.to_string(),
                new_value: defaults.disabled.to_string(),
            });
        }

        if !args.dry_run {
            profile.rate_limits = defaults;
        }
    }

    let message = if args.dry_run {
        format!("Dry run: {} change(s) would be made", changes.len())
    } else if changes.is_empty() {
        "No changes needed - already at defaults".to_string()
    } else {
        if config.is_interactive() && !args.yes {
            CliService::warning(&format!(
                "This will reset {} value(s) to defaults",
                changes.len()
            ));
        }
        require_confirmation("Proceed with reset?", args.yes, config)?;
        save_profile(&profile, profile_path)?;
        format!("Reset {} value(s) to defaults", changes.len())
    };

    let output = ResetOutput {
        reset_type,
        changes,
        message,
    };

    render_result(&CommandResult::table(output).with_title("Rate Limits Reset"));

    if !args.dry_run && config.output_format() == OutputFormat::Table {
        CliService::warning("Restart services for changes to take effect");
    }

    Ok(())
}
