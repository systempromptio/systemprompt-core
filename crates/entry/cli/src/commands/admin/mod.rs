//! `admin` command tree: privileged platform administration.
//!
//! [`AdminCommands`] groups user, agent, configuration, session, bridge,
//! access-control, and signing-key management plus the setup and bootstrap
//! flows. [`execute`] dispatches commands that resolve their own context;
//! [`execute_with_db`] serves the subset that requires a shared
//! [`systemprompt_runtime::DatabaseContext`].

pub mod access_control;
pub mod agents;
pub mod bootstrap;
pub mod bridge;
pub mod config;
pub mod keys;
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

    #[command(
        about = "Idempotently ensure the platform admin user exists with the admin role. Required \
                 by every install recipe before services start."
    )]
    Bootstrap(bootstrap::BootstrapArgs),

    #[command(subcommand, about = "Manage CLI session and profile switching")]
    Session(session::SessionCommands),

    #[command(
        subcommand,
        about = "Bridge helper enrollment (device certs, exchange codes)"
    )]
    Bridge(bridge::BridgeCommands),

    #[command(
        subcommand,
        name = "access-control",
        about = "Access-control baseline operations (DB → YAML export)"
    )]
    AccessControl(access_control::AccessControlCommands),

    #[command(
        subcommand,
        about = "RSA signing-key generation for the federated JWT plane"
    )]
    Keys(keys::KeysCommands),
}

pub async fn execute(cmd: AdminCommands, config: &CliConfig) -> Result<()> {
    match cmd {
        AdminCommands::Users(cmd) => users::execute(cmd, config).await,
        AdminCommands::Agents(cmd) => agents::execute(cmd).await,
        AdminCommands::Config(cmd) => config::execute(cmd, config).await,
        AdminCommands::Setup(args) => {
            let result = setup::execute(args, config).await?;
            render_result(&result);
            Ok(())
        },
        AdminCommands::Bootstrap(args) => {
            let result = bootstrap::execute(args, config).await?;
            render_result(&result);
            Ok(())
        },
        AdminCommands::Session(cmd) => session::execute(cmd, config).await,
        AdminCommands::Bridge(cmd) => bridge::execute(cmd, config).await,
        AdminCommands::AccessControl(cmd) => access_control::execute(cmd, config).await,
        AdminCommands::Keys(cmd) => keys::execute(cmd).await,
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
