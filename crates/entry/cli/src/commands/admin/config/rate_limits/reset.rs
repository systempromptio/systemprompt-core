//! `admin config rate-limits reset` command.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use systemprompt_config::ProfileBootstrap;
use systemprompt_logging::CliService;
use systemprompt_models::profile::RateLimitsConfig;

use super::ResetArgs;
use super::helpers::{
    collect_endpoint_changes, get_endpoint_rate, load_profile_for_edit, save_profile,
    set_endpoint_rate,
};
use crate::CliConfig;
use crate::cli_settings::OutputFormat;
use crate::interactive::{Prompter, require_confirmation};
use crate::shared::{CommandOutput, render_result};

use super::super::types::{ResetChange, ResetOutput};

pub(super) fn execute_reset(
    args: &ResetArgs,
    prompter: &dyn Prompter,
    config: &CliConfig,
) -> Result<()> {
    let profile_path = ProfileBootstrap::get_path()?;
    let mut profile = load_profile_for_edit(profile_path)?;
    let defaults = RateLimitsConfig::default();

    let (reset_type, changes) = if let Some(endpoint) = &args.endpoint {
        (
            format!("endpoint:{}", endpoint),
            reset_endpoint(&mut profile.rate_limits, &defaults, endpoint, args.dry_run)?,
        )
    } else {
        (
            "all".to_owned(),
            reset_all(&mut profile.rate_limits, defaults, args.dry_run),
        )
    };

    let message = if args.dry_run {
        format!("Dry run: {} change(s) would be made", changes.len())
    } else if changes.is_empty() {
        "No changes needed - already at defaults".to_owned()
    } else {
        if config.is_interactive() && !args.yes {
            CliService::warning(&format!(
                "This will reset {} value(s) to defaults",
                changes.len()
            ));
        }
        require_confirmation(prompter, "Proceed with reset?", args.yes, config)?;
        save_profile(&profile, profile_path)?;
        format!("Reset {} value(s) to defaults", changes.len())
    };

    let output = ResetOutput {
        reset_type,
        changes,
        message,
    };

    render_result(
        &CommandOutput::table_of(vec!["field", "old_value", "new_value"], &output.changes)
            .with_title("Rate Limits Reset"),
        config,
    );

    if !args.dry_run && config.output_format() == OutputFormat::Table {
        CliService::warning("Restart services for changes to take effect");
    }

    Ok(())
}

fn reset_endpoint(
    limits: &mut RateLimitsConfig,
    defaults: &RateLimitsConfig,
    endpoint: &str,
    dry_run: bool,
) -> Result<Vec<ResetChange>> {
    let old_value = get_endpoint_rate(limits, endpoint)?;
    let new_value = get_endpoint_rate(defaults, endpoint)?;

    let mut changes = Vec::new();
    if old_value != new_value {
        changes.push(ResetChange {
            field: format!("{}_per_second", endpoint),
            old_value: old_value.to_string(),
            new_value: new_value.to_string(),
        });
        if !dry_run {
            set_endpoint_rate(limits, endpoint, new_value)?;
        }
    }
    Ok(changes)
}

fn reset_all(
    limits: &mut RateLimitsConfig,
    defaults: RateLimitsConfig,
    dry_run: bool,
) -> Vec<ResetChange> {
    let mut changes = Vec::new();
    collect_endpoint_changes(limits, &defaults, &mut changes);

    if limits.burst_multiplier != defaults.burst_multiplier {
        changes.push(ResetChange {
            field: "burst_multiplier".to_owned(),
            old_value: limits.burst_multiplier.to_string(),
            new_value: defaults.burst_multiplier.to_string(),
        });
    }
    if limits.disabled != defaults.disabled {
        changes.push(ResetChange {
            field: "disabled".to_owned(),
            old_value: limits.disabled.to_string(),
            new_value: defaults.disabled.to_string(),
        });
    }

    if !dry_run {
        *limits = defaults;
    }
    changes
}
