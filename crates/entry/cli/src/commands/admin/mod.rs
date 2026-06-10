//! `admin` command tree: privileged platform administration.
//!
//! [`AdminCommands`] groups user, agent, configuration, session, bridge,
//! access-control, and signing-key management plus the setup and bootstrap
//! flows. On a `--database-url` invocation only the user-management subgroup
//! is served; the rest require a full profile context.

pub mod access_control;
pub mod agents;
pub mod bootstrap;
pub mod bridge;
pub mod config;
pub mod keys;
pub mod session;
pub mod setup;
pub mod users;

use anyhow::{Result, bail};
use clap::Subcommand;

use crate::context::CommandContext;
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

pub async fn execute(cmd: AdminCommands, ctx: &CommandContext) -> Result<()> {
    if ctx.is_database_scoped() && !matches!(cmd, AdminCommands::Users(_)) {
        bail!("This command requires full profile context");
    }

    match cmd {
        AdminCommands::Users(cmd) => users::execute(cmd, ctx).await,
        AdminCommands::Agents(cmd) => agents::execute(cmd, ctx).await,
        AdminCommands::Config(cmd) => config::execute(cmd, ctx).await,
        AdminCommands::Setup(args) => {
            let result = setup::execute(args, ctx).await?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
        AdminCommands::Bootstrap(args) => {
            let result = bootstrap::execute(args, &ctx.cli).await?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
        AdminCommands::Session(cmd) => session::execute(cmd, ctx).await,
        AdminCommands::Bridge(cmd) => bridge::execute(cmd, ctx).await,
        AdminCommands::AccessControl(cmd) => access_control::execute(cmd, ctx).await,
        AdminCommands::Keys(cmd) => keys::execute(cmd, ctx).await,
    }
}
