use anyhow::{bail, Result};
use clap::{Args, Subcommand};
use systemprompt_models::ProfileBootstrap;

use super::types::{
    EffectiveLimitsOutput, RateLimitsOutput, TierEffectiveLimitsOutput, TierMultipliersOutput,
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

    #[command(about = "Generate rate limits documentation")]
    Docs,
}

#[derive(Debug, Clone, Args)]
pub struct TierArgs {
    #[arg(value_name = "TIER", help = "Tier name: admin, user, a2a, mcp, service, anon")]
    pub tier: String,
}

pub fn execute(command: RateLimitsCommands, config: &CliConfig) -> Result<()> {
    match command {
        RateLimitsCommands::Show => execute_show(config),
        RateLimitsCommands::Tier(args) => execute_tier(args, config),
        RateLimitsCommands::Docs => execute_docs(),
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
        println!("\n  Note: Rate limiting is currently DISABLED");
    }

    Ok(())
}

pub fn execute_tier(args: TierArgs, config: &CliConfig) -> Result<()> {
    let profile = ProfileBootstrap::get()?;
    let limits = &profile.rate_limits;

    let multiplier = match args.tier.as_str() {
        "admin" => limits.tier_multipliers.admin,
        "user" => limits.tier_multipliers.user,
        "a2a" => limits.tier_multipliers.a2a,
        "mcp" => limits.tier_multipliers.mcp,
        "service" => limits.tier_multipliers.service,
        "anon" => limits.tier_multipliers.anon,
        _ => bail!(
            "Unknown tier: {}. Valid tiers: admin, user, a2a, mcp, service, anon",
            args.tier
        ),
    };

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
        println!("\n  Note: Rate limiting is currently DISABLED");
    }

    Ok(())
}

#[allow(clippy::print_stdout)]
pub fn execute_docs() -> Result<()> {
    let profile = ProfileBootstrap::get()?;
    let limits = &profile.rate_limits;

    println!("# Rate Limits Configuration\n");
    println!("## Base Rates\n");
    println!("| Endpoint | Base Rate |");
    println!("|----------|-----------|");
    println!("| OAuth Public | {}/s |", limits.oauth_public_per_second);
    println!("| OAuth Auth | {}/s |", limits.oauth_auth_per_second);
    println!("| Contexts | {}/s |", limits.contexts_per_second);
    println!("| Tasks | {}/s |", limits.tasks_per_second);
    println!("| Artifacts | {}/s |", limits.artifacts_per_second);
    println!(
        "| Agent Registry | {}/s |",
        limits.agent_registry_per_second
    );
    println!("| Agents | {}/s |", limits.agents_per_second);
    println!("| MCP Registry | {}/s |", limits.mcp_registry_per_second);
    println!("| MCP | {}/s |", limits.mcp_per_second);
    println!("| Stream (SSE) | {}/s |", limits.stream_per_second);
    println!("| Content | {}/s |", limits.content_per_second);

    println!("\n## Tier Multipliers\n");
    println!("| Tier | Multiplier |");
    println!("|------|------------|");
    println!("| Admin | {}x |", limits.tier_multipliers.admin);
    println!("| User | {}x |", limits.tier_multipliers.user);
    println!("| A2A | {}x |", limits.tier_multipliers.a2a);
    println!("| MCP | {}x |", limits.tier_multipliers.mcp);
    println!("| Service | {}x |", limits.tier_multipliers.service);
    println!("| Anonymous | {}x |", limits.tier_multipliers.anon);

    println!("\n## Effective Limits by Tier\n");
    println!("| Endpoint | Admin | User | Anonymous |");
    println!("|----------|-------|------|-----------|");

    let admin_mult = limits.tier_multipliers.admin;
    let user_mult = limits.tier_multipliers.user;
    let anon_mult = limits.tier_multipliers.anon;

    println!(
        "| Contexts | {}/s | {}/s | {}/s |",
        apply_multiplier(limits.contexts_per_second, admin_mult),
        apply_multiplier(limits.contexts_per_second, user_mult),
        apply_multiplier(limits.contexts_per_second, anon_mult)
    );
    println!(
        "| Tasks | {}/s | {}/s | {}/s |",
        apply_multiplier(limits.tasks_per_second, admin_mult),
        apply_multiplier(limits.tasks_per_second, user_mult),
        apply_multiplier(limits.tasks_per_second, anon_mult)
    );
    println!(
        "| Agents | {}/s | {}/s | {}/s |",
        apply_multiplier(limits.agents_per_second, admin_mult),
        apply_multiplier(limits.agents_per_second, user_mult),
        apply_multiplier(limits.agents_per_second, anon_mult)
    );
    println!(
        "| Stream (SSE) | {}/s | {}/s | {}/s |",
        apply_multiplier(limits.stream_per_second, admin_mult),
        apply_multiplier(limits.stream_per_second, user_mult),
        apply_multiplier(limits.stream_per_second, anon_mult)
    );
    println!(
        "| MCP | {}/s | {}/s | {}/s |",
        apply_multiplier(limits.mcp_per_second, admin_mult),
        apply_multiplier(limits.mcp_per_second, user_mult),
        apply_multiplier(limits.mcp_per_second, anon_mult)
    );

    println!("\n## Burst Handling\n");
    println!("Burst multiplier: {}x", limits.burst_multiplier);
    println!(
        "\nAll limits above can temporarily burst to {}x their value.",
        limits.burst_multiplier
    );

    if limits.disabled {
        println!("\n## Status\n");
        println!("**Rate limiting is currently DISABLED**");
    }

    Ok(())
}

fn apply_multiplier(base: u64, multiplier: f64) -> u64 {
    (base as f64 * multiplier).round() as u64
}
