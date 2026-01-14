use anyhow::{bail, Context, Result};
use clap::{Args, Subcommand};
use std::fs;
use systemprompt_core_logging::CliService;
use systemprompt_models::profile::{RateLimitsConfig, TierMultipliers};
use systemprompt_models::{Profile, ProfileBootstrap};

use std::path::Path;

use super::types::{
    BaseRateRow, CompareOutput, DiffEntry, DiffOutput, EffectiveLimitRow, EffectiveLimitsOutput,
    EndpointComparison, ExportOutput, ImportOutput, PresetApplyOutput, PresetInfo,
    PresetListOutput, PresetShowOutput, RateLimitStatusOutput, RateLimitsDocsOutput,
    RateLimitsOutput, ResetChange, ResetOutput, SetRateLimitOutput, TierEffectiveLimitsOutput,
    TierMultiplierRow, TierMultipliersOutput, ValidateOutput,
};
use crate::cli_settings::OutputFormat;
use crate::shared::{render_result, CommandResult};
use crate::CliConfig;

#[derive(Debug, Subcommand)]
pub enum RateLimitsCommands {
    #[command(about = "Show current rate limits configuration")]
    Show,

    #[command(about = "Show effective limits for a specific tier")]
    Tier(TierArgs),

    #[command(about = "Show rate limits documentation")]
    Docs,

    #[command(about = "Set a rate limit value")]
    Set(SetArgs),

    #[command(about = "Enable rate limiting")]
    Enable,

    #[command(about = "Disable rate limiting")]
    Disable,

    #[command(about = "Validate rate limit configuration")]
    Validate,

    #[command(about = "Compare effective limits across all tiers")]
    Compare,

    #[command(about = "Reset rate limits to defaults")]
    Reset(ResetArgs),

    #[command(subcommand, about = "Manage rate limit presets")]
    Preset(PresetCommands),

    #[command(about = "Export rate limits to file")]
    Export(ExportArgs),

    #[command(about = "Import rate limits from file")]
    Import(ImportArgs),

    #[command(about = "Compare rate limits with defaults or file")]
    Diff(DiffArgs),
}

#[derive(Debug, Clone, Args)]
pub struct TierArgs {
    #[arg(value_name = "TIER", help = "Tier name: admin, user, a2a, mcp, service, anon")]
    pub tier: String,
}

#[derive(Debug, Clone, Args)]
pub struct SetArgs {
    #[arg(long, help = "Endpoint to modify: oauth_public, oauth_auth, contexts, tasks, artifacts, agent_registry, agents, mcp_registry, mcp, stream, content")]
    pub endpoint: Option<String>,

    #[arg(long, help = "Rate per second (requires --endpoint)")]
    pub rate: Option<u64>,

    #[arg(long, help = "Tier to modify multiplier: admin, user, a2a, mcp, service, anon")]
    pub tier: Option<String>,

    #[arg(long, help = "Multiplier value (requires --tier)")]
    pub multiplier: Option<f64>,

    #[arg(long, help = "Burst multiplier value")]
    pub burst: Option<u64>,
}

#[derive(Debug, Clone, Args)]
pub struct ResetArgs {
    #[arg(short = 'y', long, help = "Skip confirmation")]
    pub yes: bool,

    #[arg(long, help = "Preview changes without applying")]
    pub dry_run: bool,

    #[arg(long, help = "Reset only this endpoint")]
    pub endpoint: Option<String>,

    #[arg(long, help = "Reset only this tier multiplier")]
    pub tier: Option<String>,
}

#[derive(Debug, Subcommand)]
pub enum PresetCommands {
    #[command(about = "List available presets")]
    List,

    #[command(about = "Show preset configuration")]
    Show(PresetShowArgs),

    #[command(about = "Apply a preset")]
    Apply(PresetApplyArgs),
}

#[derive(Debug, Clone, Args)]
pub struct PresetShowArgs {
    #[arg(help = "Preset name: development, production, high-traffic")]
    pub name: String,
}

#[derive(Debug, Clone, Args)]
pub struct PresetApplyArgs {
    #[arg(help = "Preset name: development, production, high-traffic")]
    pub name: String,

    #[arg(short = 'y', long, help = "Skip confirmation")]
    pub yes: bool,
}

#[derive(Debug, Clone, Args)]
pub struct ExportArgs {
    #[arg(long, short = 'o', help = "Output file path")]
    pub output: String,

    #[arg(long, default_value = "yaml", help = "Format: yaml, json")]
    pub format: String,
}

#[derive(Debug, Clone, Args)]
pub struct ImportArgs {
    #[arg(long, short = 'f', help = "Input file path")]
    pub file: String,

    #[arg(short = 'y', long, help = "Skip confirmation")]
    pub yes: bool,
}

#[derive(Debug, Clone, Args)]
pub struct DiffArgs {
    #[arg(long, help = "Compare with defaults")]
    pub defaults: bool,

    #[arg(long, short = 'f', help = "Compare with file")]
    pub file: Option<String>,
}

pub fn execute(command: RateLimitsCommands, config: &CliConfig) -> Result<()> {
    match command {
        RateLimitsCommands::Show => execute_show(config),
        RateLimitsCommands::Tier(args) => execute_tier(args, config),
        RateLimitsCommands::Docs => execute_docs(config),
        RateLimitsCommands::Set(args) => execute_set(args, config),
        RateLimitsCommands::Enable => execute_enable(config),
        RateLimitsCommands::Disable => execute_disable(config),
        RateLimitsCommands::Validate => execute_validate(config),
        RateLimitsCommands::Compare => execute_compare(config),
        RateLimitsCommands::Reset(args) => execute_reset(args, config),
        RateLimitsCommands::Preset(cmd) => execute_preset(cmd, config),
        RateLimitsCommands::Export(args) => execute_export(args, config),
        RateLimitsCommands::Import(args) => execute_import(args, config),
        RateLimitsCommands::Diff(args) => execute_diff(args, config),
    }
}

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

pub fn execute_set(args: SetArgs, config: &CliConfig) -> Result<()> {
    // Validate args
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
            message: format!("Updated {} tier multiplier: {:.1}x -> {:.1}x", tier, old_value, multiplier),
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
    render_result(&CommandResult::text(output.clone()).with_title("Rate Limit Updated"));

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

pub fn execute_validate(config: &CliConfig) -> Result<()> {
    let profile = ProfileBootstrap::get()?;
    let limits = &profile.rate_limits;

    let mut errors: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();

    // Check for zero rates
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

    // Check tier multiplier hierarchy
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

    // Check for negative or zero multipliers
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

    // Check burst multiplier
    if limits.burst_multiplier == 0 {
        errors.push("burst_multiplier is 0".to_string());
    }
    if limits.burst_multiplier > 10 {
        warnings.push(format!(
            "burst_multiplier {} exceeds recommended maximum of 10",
            limits.burst_multiplier
        ));
    }

    // Check if disabled
    if limits.disabled {
        warnings.push("Rate limiting is currently DISABLED".to_string());
    }

    let valid = errors.is_empty();
    let output = ValidateOutput {
        valid,
        errors,
        warnings,
    };

    render_result(&CommandResult::card(output.clone()).with_title("Rate Limits Validation"));

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

pub fn execute_reset(args: ResetArgs, config: &CliConfig) -> Result<()> {
    if !args.yes && !args.dry_run && !config.is_interactive() {
        bail!("--yes or --dry-run is required in non-interactive mode");
    }

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
        // Compare all fields and collect changes
        collect_endpoint_changes(limits, &defaults, &mut changes);
        collect_tier_changes(&limits.tier_multipliers, &defaults.tier_multipliers, &mut changes);

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
        if !args.yes && config.is_interactive() {
            CliService::warning(&format!("This will reset {} value(s) to defaults", changes.len()));
            if !CliService::confirm("Proceed with reset?")? {
                CliService::info("Reset cancelled");
                return Ok(());
            }
        }
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

// === Helper Functions ===

fn apply_multiplier(base: u64, multiplier: f64) -> u64 {
    (base as f64 * multiplier).round() as u64
}

fn get_tier_multiplier(tiers: &TierMultipliers, tier: &str) -> Result<f64> {
    match tier {
        "admin" => Ok(tiers.admin),
        "user" => Ok(tiers.user),
        "a2a" => Ok(tiers.a2a),
        "mcp" => Ok(tiers.mcp),
        "service" => Ok(tiers.service),
        "anon" => Ok(tiers.anon),
        _ => bail!(
            "Unknown tier: {}. Valid tiers: admin, user, a2a, mcp, service, anon",
            tier
        ),
    }
}

fn set_tier_multiplier(tiers: &mut TierMultipliers, tier: &str, value: f64) -> Result<()> {
    match tier {
        "admin" => tiers.admin = value,
        "user" => tiers.user = value,
        "a2a" => tiers.a2a = value,
        "mcp" => tiers.mcp = value,
        "service" => tiers.service = value,
        "anon" => tiers.anon = value,
        _ => bail!(
            "Unknown tier: {}. Valid tiers: admin, user, a2a, mcp, service, anon",
            tier
        ),
    }
    Ok(())
}

fn get_endpoint_rate(limits: &RateLimitsConfig, endpoint: &str) -> Result<u64> {
    match endpoint {
        "oauth_public" => Ok(limits.oauth_public_per_second),
        "oauth_auth" => Ok(limits.oauth_auth_per_second),
        "contexts" => Ok(limits.contexts_per_second),
        "tasks" => Ok(limits.tasks_per_second),
        "artifacts" => Ok(limits.artifacts_per_second),
        "agent_registry" => Ok(limits.agent_registry_per_second),
        "agents" => Ok(limits.agents_per_second),
        "mcp_registry" => Ok(limits.mcp_registry_per_second),
        "mcp" => Ok(limits.mcp_per_second),
        "stream" => Ok(limits.stream_per_second),
        "content" => Ok(limits.content_per_second),
        _ => bail!(
            "Unknown endpoint: {}. Valid endpoints: oauth_public, oauth_auth, contexts, tasks, artifacts, agent_registry, agents, mcp_registry, mcp, stream, content",
            endpoint
        ),
    }
}

fn set_endpoint_rate(limits: &mut RateLimitsConfig, endpoint: &str, value: u64) -> Result<()> {
    match endpoint {
        "oauth_public" => limits.oauth_public_per_second = value,
        "oauth_auth" => limits.oauth_auth_per_second = value,
        "contexts" => limits.contexts_per_second = value,
        "tasks" => limits.tasks_per_second = value,
        "artifacts" => limits.artifacts_per_second = value,
        "agent_registry" => limits.agent_registry_per_second = value,
        "agents" => limits.agents_per_second = value,
        "mcp_registry" => limits.mcp_registry_per_second = value,
        "mcp" => limits.mcp_per_second = value,
        "stream" => limits.stream_per_second = value,
        "content" => limits.content_per_second = value,
        _ => bail!(
            "Unknown endpoint: {}. Valid endpoints: oauth_public, oauth_auth, contexts, tasks, artifacts, agent_registry, agents, mcp_registry, mcp, stream, content",
            endpoint
        ),
    }
    Ok(())
}

fn load_profile_for_edit(path: &str) -> Result<Profile> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read profile: {}", path))?;
    let profile: Profile = serde_yaml::from_str(&content)
        .with_context(|| format!("Failed to parse profile: {}", path))?;
    Ok(profile)
}

fn save_profile(profile: &Profile, path: &str) -> Result<()> {
    let content = serde_yaml::to_string(profile)
        .context("Failed to serialize profile")?;
    fs::write(path, content)
        .with_context(|| format!("Failed to write profile: {}", path))?;
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

fn collect_endpoint_changes(
    current: &RateLimitsConfig,
    defaults: &RateLimitsConfig,
    changes: &mut Vec<ResetChange>,
) {
    let endpoints = [
        ("oauth_public_per_second", current.oauth_public_per_second, defaults.oauth_public_per_second),
        ("oauth_auth_per_second", current.oauth_auth_per_second, defaults.oauth_auth_per_second),
        ("contexts_per_second", current.contexts_per_second, defaults.contexts_per_second),
        ("tasks_per_second", current.tasks_per_second, defaults.tasks_per_second),
        ("artifacts_per_second", current.artifacts_per_second, defaults.artifacts_per_second),
        ("agent_registry_per_second", current.agent_registry_per_second, defaults.agent_registry_per_second),
        ("agents_per_second", current.agents_per_second, defaults.agents_per_second),
        ("mcp_registry_per_second", current.mcp_registry_per_second, defaults.mcp_registry_per_second),
        ("mcp_per_second", current.mcp_per_second, defaults.mcp_per_second),
        ("stream_per_second", current.stream_per_second, defaults.stream_per_second),
        ("content_per_second", current.content_per_second, defaults.content_per_second),
    ];

    for (name, current_val, default_val) in endpoints {
        if current_val != default_val {
            changes.push(ResetChange {
                field: name.to_string(),
                old_value: current_val.to_string(),
                new_value: default_val.to_string(),
            });
        }
    }
}

fn collect_tier_changes(
    current: &TierMultipliers,
    defaults: &TierMultipliers,
    changes: &mut Vec<ResetChange>,
) {
    let tiers = [
        ("tier_multipliers.admin", current.admin, defaults.admin),
        ("tier_multipliers.user", current.user, defaults.user),
        ("tier_multipliers.a2a", current.a2a, defaults.a2a),
        ("tier_multipliers.mcp", current.mcp, defaults.mcp),
        ("tier_multipliers.service", current.service, defaults.service),
        ("tier_multipliers.anon", current.anon, defaults.anon),
    ];

    for (name, current_val, default_val) in tiers {
        if (current_val - default_val).abs() > f64::EPSILON {
            changes.push(ResetChange {
                field: name.to_string(),
                old_value: format!("{:.1}", current_val),
                new_value: format!("{:.1}", default_val),
            });
        }
    }
}

// === Preset Commands ===

fn execute_preset(command: PresetCommands, config: &CliConfig) -> Result<()> {
    match command {
        PresetCommands::List => execute_preset_list(config),
        PresetCommands::Show(args) => execute_preset_show(args, config),
        PresetCommands::Apply(args) => execute_preset_apply(args, config),
    }
}

fn execute_preset_list(_config: &CliConfig) -> Result<()> {
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

    Ok(())
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

fn execute_preset_apply(args: PresetApplyArgs, config: &CliConfig) -> Result<()> {
    if !args.yes && !config.is_interactive() {
        bail!("--yes is required in non-interactive mode");
    }

    let preset_config = get_preset_config(&args.name)?;

    if !args.yes && config.is_interactive() {
        CliService::warning(&format!(
            "This will apply the '{}' preset to rate limits",
            args.name
        ));
        if !CliService::confirm("Proceed with preset application?")? {
            CliService::info("Preset application cancelled");
            return Ok(());
        }
    }

    let profile_path = ProfileBootstrap::get_path()?;
    let mut profile = load_profile_for_edit(profile_path)?;

    // Collect changes
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

// === Export/Import Commands ===

fn execute_export(args: ExportArgs, config: &CliConfig) -> Result<()> {
    let profile = ProfileBootstrap::get()?;
    let limits = &profile.rate_limits;

    let content = match args.format.as_str() {
        "yaml" | "yml" => {
            serde_yaml::to_string(limits).context("Failed to serialize rate limits to YAML")?
        }
        "json" => {
            serde_json::to_string_pretty(limits).context("Failed to serialize rate limits to JSON")?
        }
        _ => bail!("Unknown format: {}. Valid formats: yaml, json", args.format),
    };

    fs::write(&args.output, &content)
        .with_context(|| format!("Failed to write to file: {}", args.output))?;

    let output = ExportOutput {
        path: args.output.clone(),
        format: args.format.clone(),
        message: format!("Exported rate limits to {}", args.output),
    };

    render_result(&CommandResult::text(output).with_title("Rate Limits Exported"));

    if config.output_format() == OutputFormat::Table {
        CliService::success(&format!("Exported to {}", args.output));
    }

    Ok(())
}

fn execute_import(args: ImportArgs, config: &CliConfig) -> Result<()> {
    if !args.yes && !config.is_interactive() {
        bail!("--yes is required in non-interactive mode");
    }

    let path = Path::new(&args.file);
    if !path.exists() {
        bail!("File not found: {}", args.file);
    }

    let content =
        fs::read_to_string(&args.file).with_context(|| format!("Failed to read file: {}", args.file))?;

    let format = if args.file.ends_with(".json") {
        "json"
    } else {
        "yaml"
    };

    let new_limits: RateLimitsConfig = match format {
        "json" => serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse JSON from: {}", args.file))?,
        _ => serde_yaml::from_str(&content)
            .with_context(|| format!("Failed to parse YAML from: {}", args.file))?,
    };

    if !args.yes && config.is_interactive() {
        CliService::warning(&format!("This will import rate limits from {}", args.file));
        if !CliService::confirm("Proceed with import?")? {
            CliService::info("Import cancelled");
            return Ok(());
        }
    }

    let profile_path = ProfileBootstrap::get_path()?;
    let mut profile = load_profile_for_edit(profile_path)?;
    profile.rate_limits = new_limits;
    save_profile(&profile, profile_path)?;

    let output = ImportOutput {
        path: args.file.clone(),
        changes: vec![],
        message: format!("Imported rate limits from {}", args.file),
    };

    render_result(&CommandResult::text(output).with_title("Rate Limits Imported"));

    if config.output_format() == OutputFormat::Table {
        CliService::warning("Restart services for changes to take effect");
    }

    Ok(())
}

// === Diff Command ===

fn execute_diff(args: DiffArgs, config: &CliConfig) -> Result<()> {
    let profile = ProfileBootstrap::get()?;
    let current = &profile.rate_limits;

    let (compare_with, source) = if args.defaults {
        (RateLimitsConfig::default(), "defaults".to_string())
    } else if let Some(file_path) = &args.file {
        let content = fs::read_to_string(file_path)
            .with_context(|| format!("Failed to read file: {}", file_path))?;

        let format = if file_path.ends_with(".json") {
            "json"
        } else {
            "yaml"
        };

        let limits: RateLimitsConfig = match format {
            "json" => serde_json::from_str(&content)
                .with_context(|| format!("Failed to parse JSON from: {}", file_path))?,
            _ => serde_yaml::from_str(&content)
                .with_context(|| format!("Failed to parse YAML from: {}", file_path))?,
        };

        (limits, file_path.clone())
    } else {
        bail!("Must specify --defaults or --file");
    };

    let mut differences: Vec<DiffEntry> = Vec::new();

    // Compare all fields
    add_diff_if_different(&mut differences, "disabled", current.disabled, compare_with.disabled);
    add_diff_if_different(
        &mut differences,
        "oauth_public_per_second",
        current.oauth_public_per_second,
        compare_with.oauth_public_per_second,
    );
    add_diff_if_different(
        &mut differences,
        "oauth_auth_per_second",
        current.oauth_auth_per_second,
        compare_with.oauth_auth_per_second,
    );
    add_diff_if_different(
        &mut differences,
        "contexts_per_second",
        current.contexts_per_second,
        compare_with.contexts_per_second,
    );
    add_diff_if_different(
        &mut differences,
        "tasks_per_second",
        current.tasks_per_second,
        compare_with.tasks_per_second,
    );
    add_diff_if_different(
        &mut differences,
        "artifacts_per_second",
        current.artifacts_per_second,
        compare_with.artifacts_per_second,
    );
    add_diff_if_different(
        &mut differences,
        "agent_registry_per_second",
        current.agent_registry_per_second,
        compare_with.agent_registry_per_second,
    );
    add_diff_if_different(
        &mut differences,
        "agents_per_second",
        current.agents_per_second,
        compare_with.agents_per_second,
    );
    add_diff_if_different(
        &mut differences,
        "mcp_registry_per_second",
        current.mcp_registry_per_second,
        compare_with.mcp_registry_per_second,
    );
    add_diff_if_different(
        &mut differences,
        "mcp_per_second",
        current.mcp_per_second,
        compare_with.mcp_per_second,
    );
    add_diff_if_different(
        &mut differences,
        "stream_per_second",
        current.stream_per_second,
        compare_with.stream_per_second,
    );
    add_diff_if_different(
        &mut differences,
        "content_per_second",
        current.content_per_second,
        compare_with.content_per_second,
    );
    add_diff_if_different(
        &mut differences,
        "burst_multiplier",
        current.burst_multiplier,
        compare_with.burst_multiplier,
    );

    // Compare tier multipliers
    add_diff_if_different_f64(
        &mut differences,
        "tier_multipliers.admin",
        current.tier_multipliers.admin,
        compare_with.tier_multipliers.admin,
    );
    add_diff_if_different_f64(
        &mut differences,
        "tier_multipliers.user",
        current.tier_multipliers.user,
        compare_with.tier_multipliers.user,
    );
    add_diff_if_different_f64(
        &mut differences,
        "tier_multipliers.a2a",
        current.tier_multipliers.a2a,
        compare_with.tier_multipliers.a2a,
    );
    add_diff_if_different_f64(
        &mut differences,
        "tier_multipliers.mcp",
        current.tier_multipliers.mcp,
        compare_with.tier_multipliers.mcp,
    );
    add_diff_if_different_f64(
        &mut differences,
        "tier_multipliers.service",
        current.tier_multipliers.service,
        compare_with.tier_multipliers.service,
    );
    add_diff_if_different_f64(
        &mut differences,
        "tier_multipliers.anon",
        current.tier_multipliers.anon,
        compare_with.tier_multipliers.anon,
    );

    let output = DiffOutput {
        source,
        differences: differences.clone(),
        identical: differences.is_empty(),
    };

    render_result(&CommandResult::table(output).with_title("Rate Limits Diff"));

    if config.output_format() == OutputFormat::Table {
        if differences.is_empty() {
            CliService::success("No differences found");
        } else {
            CliService::info(&format!("{} difference(s) found", differences.len()));
        }
    }

    Ok(())
}

fn add_diff_if_different<T: std::fmt::Display + PartialEq>(
    diffs: &mut Vec<DiffEntry>,
    field: &str,
    current: T,
    compare: T,
) {
    if current != compare {
        diffs.push(DiffEntry {
            field: field.to_string(),
            current: current.to_string(),
            other: compare.to_string(),
        });
    }
}

fn add_diff_if_different_f64(diffs: &mut Vec<DiffEntry>, field: &str, current: f64, compare: f64) {
    if (current - compare).abs() > f64::EPSILON {
        diffs.push(DiffEntry {
            field: field.to_string(),
            current: format!("{:.1}", current),
            other: format!("{:.1}", compare),
        });
    }
}
