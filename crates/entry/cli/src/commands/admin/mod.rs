//! `admin` command tree: privileged platform administration.
//!
//! [`AdminCommands`] groups user, agent, configuration, session, bridge,
//! access-control, and signing-key management plus the setup and bootstrap
//! flows. On a `--database-url` invocation only the user-management subgroup
//! is served; the rest require a full profile context.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod access_control;
pub mod agents;
pub mod bootstrap;
pub mod bridge;
pub mod config;
pub mod evals;
pub mod identity;
pub mod keys;
pub mod session;
pub mod setup;
pub mod users;

use anyhow::Result;
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

    #[command(subcommand, about = "Evaluation runs over production AI traffic")]
    Evals(evals::EvalsCommands),

    #[command(about = "Interactive setup wizard for local development environment")]
    Setup(setup::SetupArgs),

    #[command(
        about = "Idempotently ensure the system admin user exists with the admin role. Required \
                 by every install recipe before services start. Note: the admin ROLE only — \
                 deployments that derive platform admin from organization membership grant that \
                 half themselves (at boot, or via their own tooling)."
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

    #[command(
        subcommand,
        about = "Replica identity secrets shared by every node of a deployment"
    )]
    Identity(identity::IdentityCommands),
}

pub async fn execute(cmd: AdminCommands, ctx: &CommandContext) -> Result<()> {
    // Why: Session is exempt alongside Users — `session login` against a cloud
    // profile with external_db_access runs exactly in this database-scoped
    // mode (profile + secrets + external DB URL, no local /app paths), and it
    // is the command that mints the token remote routing needs. Refusing it
    // here made cloud login circular: the error told the operator to run the
    // very command being refused.
    if ctx.is_database_scoped()
        && !matches!(cmd, AdminCommands::Users(_) | AdminCommands::Session(_))
    {
        return Err(crate::shared::database_scoped_command_error());
    }

    match cmd {
        AdminCommands::Users(cmd) => users::execute(cmd, ctx).await,
        AdminCommands::Agents(cmd) => Box::pin(agents::execute(cmd, ctx)).await,
        AdminCommands::Config(cmd) => config::execute(cmd, ctx).await,
        AdminCommands::Evals(cmd) => Box::pin(evals::execute(cmd, ctx)).await,
        AdminCommands::Setup(args) => {
            let result = Box::pin(setup::execute(args, ctx)).await?;
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
        AdminCommands::Identity(cmd) => identity::execute(cmd, ctx),
    }
}
