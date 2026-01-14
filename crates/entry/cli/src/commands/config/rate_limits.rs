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
