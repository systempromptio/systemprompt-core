use anyhow::{bail, Result};
use systemprompt_logging::CliService;
use systemprompt_models::profile::{RateLimitsConfig, TierMultipliers};
use systemprompt_models::ProfileBootstrap;

use super::helpers::{
    collect_endpoint_changes, collect_tier_changes, load_profile_for_edit, save_profile,
};
use super::{PresetApplyArgs, PresetCommands, PresetShowArgs};
use crate::cli_settings::OutputFormat;
use crate::interactive::require_confirmation;
use crate::shared::{render_result, CommandResult};
use crate::CliConfig;

use super::super::types::{
    PresetApplyOutput, PresetInfo, PresetListOutput, PresetShowOutput, RateLimitsOutput,
    ResetChange, TierMultipliersOutput,
};

pub fn execute_preset(command: PresetCommands, config: &CliConfig) -> Result<()> {
    match command {
        PresetCommands::List => {
            execute_preset_list(config);
            Ok(())
        },
        PresetCommands::Show(args) => execute_preset_show(args, config),
        PresetCommands::Apply(args) => execute_preset_apply(&args, config),
    }
}

fn execute_preset_list(_config: &CliConfig) {
    let presets = vec![
        PresetInfo {
            name: "development".to_string(),
            description: "Relaxed limits for local development".to_string(),
            builtin: true,
        },
        PresetInfo {
            name: "production".to_string(),
            description: "Balanced limits for production workloads".to_string(),
            builtin: true,
        },
        PresetInfo {
            name: "high-traffic".to_string(),
            description: "Strict limits for high-traffic environments".to_string(),
            builtin: true,
        },
    ];

    let output = PresetListOutput { presets };
    render_result(&CommandResult::table(output).with_title("Available Presets"));
}

fn execute_preset_show(args: PresetShowArgs, _config: &CliConfig) -> Result<()> {
    let limits = get_preset_config(&args.name)?;
    let description = get_preset_description(&args.name)?;

    let output = PresetShowOutput {
        name: args.name,
        description,
        config: RateLimitsOutput {
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
        },
    };

    render_result(&CommandResult::card(output).with_title("Preset Configuration"));

    Ok(())
}

fn get_preset_description(name: &str) -> Result<String> {
    match name {
        "development" => Ok("Relaxed limits for local development".to_string()),
        "production" => Ok("Balanced limits for production workloads".to_string()),
        "high-traffic" => Ok("Strict limits for high-traffic environments".to_string()),
        _ => bail!(
            "Unknown preset: {}. Valid presets: development, production, high-traffic",
            name
        ),
    }
}

fn execute_preset_apply(args: &PresetApplyArgs, config: &CliConfig) -> Result<()> {
    let preset_config = get_preset_config(&args.name)?;

    if config.is_interactive() && !args.yes {
        CliService::warning(&format!(
            "This will apply the '{}' preset to rate limits",
            args.name
        ));
    }
    require_confirmation("Proceed with preset application?", args.yes, config)?;

    let profile_path = ProfileBootstrap::get_path()?;
    let mut profile = load_profile_for_edit(profile_path)?;

    let mut changes: Vec<ResetChange> = Vec::new();
    collect_endpoint_changes(&profile.rate_limits, &preset_config, &mut changes);
    collect_tier_changes(
        &profile.rate_limits.tier_multipliers,
        &preset_config.tier_multipliers,
        &mut changes,
    );

    if profile.rate_limits.burst_multiplier != preset_config.burst_multiplier {
        changes.push(ResetChange {
            field: "burst_multiplier".to_string(),
            old_value: profile.rate_limits.burst_multiplier.to_string(),
            new_value: preset_config.burst_multiplier.to_string(),
        });
    }
    if profile.rate_limits.disabled != preset_config.disabled {
        changes.push(ResetChange {
            field: "disabled".to_string(),
            old_value: profile.rate_limits.disabled.to_string(),
            new_value: preset_config.disabled.to_string(),
        });
    }

    profile.rate_limits = preset_config;
    save_profile(&profile, profile_path)?;

    let output = PresetApplyOutput {
        preset: args.name.clone(),
        changes,
        message: format!("Applied '{}' preset successfully", args.name),
    };

    render_result(&CommandResult::table(output).with_title("Preset Applied"));

    if config.output_format() == OutputFormat::Table {
        CliService::warning("Restart services for changes to take effect");
    }

    Ok(())
}

fn get_preset_config(name: &str) -> Result<RateLimitsConfig> {
    match name {
        "development" => Ok(RateLimitsConfig {
            disabled: false,
            oauth_public_per_second: 50,
            oauth_auth_per_second: 20,
            contexts_per_second: 100,
            tasks_per_second: 100,
            artifacts_per_second: 100,
            agent_registry_per_second: 50,
            agents_per_second: 100,
            mcp_registry_per_second: 50,
            mcp_per_second: 100,
            stream_per_second: 50,
            content_per_second: 200,
            burst_multiplier: 5,
            tier_multipliers: TierMultipliers {
                admin: 10.0,
                user: 2.0,
                a2a: 3.0,
                mcp: 3.0,
                service: 5.0,
                anon: 0.5,
            },
        }),
        "production" => Ok(RateLimitsConfig::default()),
        "high-traffic" => Ok(RateLimitsConfig {
            disabled: false,
            oauth_public_per_second: 5,
            oauth_auth_per_second: 2,
            contexts_per_second: 10,
            tasks_per_second: 10,
            artifacts_per_second: 10,
            agent_registry_per_second: 5,
            agents_per_second: 10,
            mcp_registry_per_second: 5,
            mcp_per_second: 10,
            stream_per_second: 5,
            content_per_second: 20,
            burst_multiplier: 2,
            tier_multipliers: TierMultipliers {
                admin: 5.0,
                user: 1.0,
                a2a: 1.5,
                mcp: 1.5,
                service: 2.0,
                anon: 0.2,
            },
        }),
        _ => bail!(
            "Unknown preset: {}. Valid presets: development, production, high-traffic",
            name
        ),
    }
}
