//! `admin config` command tree: inspect and edit profile/service configuration.
//!
//! [`ConfigCommands`] dispatches to the show, list, and validate handlers plus
//! the rate-limit, server, runtime, security, paths, and provider sub-trees.
//! Each sub-module owns its clap surface and renders results through
//! [`crate::shared::render_result`].

pub mod catalog;
pub mod config_section;
pub mod gateway;
pub mod governance;
pub mod list;
pub mod paths;
pub mod profile_io;
pub mod provider;
pub mod reconcile;
pub mod rate_limit_types;
pub mod rate_limits;
pub mod runtime;
pub mod secret;
pub mod security;
pub mod server;
pub mod show;
pub mod types;
pub mod validate;

use anyhow::Result;
use clap::Subcommand;

use crate::CliConfig;
use crate::cli_settings::get_global_config;
use crate::shared::render_result;

#[derive(Debug, Subcommand)]
pub enum ConfigCommands {
    #[command(about = "Show configuration overview")]
    Show,

    #[command(about = "List all configuration files")]
    List(list::ListArgs),

    #[command(about = "Validate configuration files")]
    Validate(validate::ValidateArgs),

    #[command(subcommand, about = "Rate limit configuration")]
    RateLimits(rate_limits::RateLimitsCommands),

    #[command(subcommand, about = "Server configuration")]
    Server(server::ServerCommands),

    #[command(subcommand, about = "Runtime configuration")]
    Runtime(runtime::RuntimeCommands),

    #[command(subcommand, about = "Security configuration")]
    Security(security::SecurityCommands),

    #[command(subcommand, about = "Paths configuration")]
    Paths(paths::PathsCommands),

    #[command(subcommand, about = "AI provider configuration")]
    Provider(provider::ProviderCommands),

    #[command(subcommand, about = "Gateway configuration (routes, default provider)")]
    Gateway(gateway::GatewayCommands),

    #[command(subcommand, about = "Governance authorization hook configuration")]
    Governance(governance::GovernanceCommands),

    #[command(
        subcommand,
        about = "Provider registry (profile.providers: providers, models)"
    )]
    Catalog(catalog::CatalogCommands),

    #[command(subcommand, about = "Profile secrets")]
    Secret(secret::SecretCommands),
}

pub async fn execute(command: ConfigCommands, config: &CliConfig) -> Result<()> {
    match command {
        ConfigCommands::Show => {
            let result = show::execute(config)?;
            render_result(&result);
            Ok(())
        },
        ConfigCommands::List(args) => {
            let result = list::execute(args, config);
            render_result(&result);
            Ok(())
        },
        ConfigCommands::Validate(args) => {
            let result = validate::execute(&args, config)?;
            render_result(&result);
            Ok(())
        },
        ConfigCommands::RateLimits(cmd) => rate_limits::execute(cmd, config),
        ConfigCommands::Server(ref cmd) => server::execute(cmd, config),
        ConfigCommands::Runtime(cmd) => runtime::execute(cmd, config),
        ConfigCommands::Security(ref cmd) => security::execute(cmd, config),
        ConfigCommands::Paths(cmd) => paths::execute(cmd, config),
        ConfigCommands::Provider(cmd) => provider::execute(cmd, config),
        ConfigCommands::Gateway(ref cmd) => gateway::execute(cmd, config).await,
        ConfigCommands::Governance(ref cmd) => governance::execute(cmd, config),
        ConfigCommands::Catalog(ref cmd) => catalog::execute(cmd, config).await,
        ConfigCommands::Secret(ref cmd) => secret::execute(cmd, config),
    }
}

pub fn execute_default() -> Result<()> {
    let config = get_global_config();
    let result = show::execute(&config)?;
    render_result(&result);
    Ok(())
}
