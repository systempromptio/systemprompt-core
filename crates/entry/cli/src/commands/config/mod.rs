pub mod rate_limits;
pub mod types;

use anyhow::Result;
use clap::Subcommand;

use crate::cli_settings::get_global_config;
use crate::CliConfig;

#[derive(Debug, Subcommand)]
pub enum ConfigCommands {
    #[command(subcommand, about = "Rate limit configuration")]
    RateLimits(rate_limits::RateLimitsCommands),
}

pub fn execute(command: ConfigCommands, config: &CliConfig) -> Result<()> {
    match command {
        ConfigCommands::RateLimits(cmd) => rate_limits::execute(cmd, config),
    }
}

pub fn execute_default() -> Result<()> {
    let config = get_global_config();
    rate_limits::execute_show(&config)
}
