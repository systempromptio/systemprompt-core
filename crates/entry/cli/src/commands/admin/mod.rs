pub mod agents;
pub mod config;
pub mod cowork;
pub mod session;
pub mod setup;
pub mod users;

use anyhow::Result;
use clap::Subcommand;
use systemprompt_runtime::DatabaseContext;

use crate::CliConfig;
use crate::shared::render_result;

#[derive(Debug, Subcommand)]
pub enum AdminCommands {
    #[command(subcommand, about = "User management and IP banning")]
    Users(users::UsersCommands),

    #[command(subcommand, about = "Agent management")]
    Agents(agents::AgentsCommands),

    #[command(subcommand, about = "Configuration management and rate limits")]
    Config(config::ConfigCommands),

    #[command(about = "Interactive setup wizard for local development environment")]
    Setup(setup::SetupArgs),

    #[command(subcommand, about = "Manage CLI session and profile switching")]
    Session(session::SessionCommands),

    #[command(
        subcommand,
        about = "Cowork helper enrollment (device certs, exchange codes)"
    )]
    Cowork(cowork::CoworkCommands),
}

pub async fn execute(cmd: AdminCommands, config: &CliConfig) -> Result<()> {
    match cmd {
        AdminCommands::Users(cmd) => users::execute(cmd, config).await,
        AdminCommands::Agents(cmd) => agents::execute(cmd).await,
        AdminCommands::Config(cmd) => config::execute(cmd, config),
        AdminCommands::Setup(args) => {
            let result = setup::execute(args, config).await?;
            render_result(&result);
            Ok(())
        },
        AdminCommands::Session(cmd) => session::execute(cmd, config).await,
        AdminCommands::Cowork(cmd) => cowork::execute(cmd, config).await,
    }
}

pub async fn execute_with_db(
    cmd: AdminCommands,
    db_ctx: &DatabaseContext,
    config: &CliConfig,
) -> Result<()> {
    match cmd {
        AdminCommands::Users(cmd) => users::execute_with_db(cmd, db_ctx, config).await,
        _ => anyhow::bail!("This command requires full profile context"),
    }
}
