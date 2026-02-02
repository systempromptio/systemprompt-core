pub mod paths;
pub mod provider;
pub mod rate_limits;
pub mod runtime;
pub mod security;
pub mod server;
pub mod show;
pub mod types;

use anyhow::Result;
use clap::Subcommand;

use crate::cli_settings::get_global_config;
use crate::CliConfig;

#[derive(Debug, Subcommand)]
pub enum ConfigCommands {
    #[command(about = "Show configuration overview")]
    Show,

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
}

pub fn execute(command: ConfigCommands, config: &CliConfig) -> Result<()> {
    match command {
        ConfigCommands::Show => show::execute(config),
        ConfigCommands::RateLimits(cmd) => rate_limits::execute(cmd, config),
        ConfigCommands::Server(ref cmd) => server::execute(cmd, config),
        ConfigCommands::Runtime(cmd) => runtime::execute(cmd, config),
        ConfigCommands::Security(ref cmd) => security::execute(cmd, config),
        ConfigCommands::Paths(cmd) => paths::execute(cmd, config),
        ConfigCommands::Provider(cmd) => provider::execute(cmd, config),
    }
}

pub fn execute_default() -> Result<()> {
    let config = get_global_config();
    show::execute(&config)
}
