//! `admin` command tree: privileged platform administration.
//!
//! [`AdminCommands`] groups user, configuration, session, bridge,
//! access-control, signing-key, issue-report triage and licence check-in
//! management plus the setup and bootstrap flows and the agent and MCP child
//! hosts. On a `--database-url`
//! invocation only the user-management subgroup is served; the rest require a
//! full profile context.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod access_control;
pub mod agent_host;
pub mod bootstrap;
pub mod bridge;
pub mod config;
pub mod identity;
pub mod issues;
pub mod keys;
pub mod licence;
pub mod mcp;
pub mod session;
pub mod setup;
mod state;
pub mod users;

use crate::kernel::KernelHost;
use anyhow::Result;
use clap::Subcommand;

use systemprompt_extension::cli::{CommandDescriptor, CommandOutput, DataImpact, DescribeCommand};

#[derive(Debug, Subcommand)]
pub enum AdminCommands {
    #[command(subcommand, about = "User management and IP banning")]
    Users(users::UsersCommands),

    #[command(subcommand, about = "Configuration management and rate limits")]
    Config(config::ConfigCommands),

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

    #[command(subcommand, about = "Issue report triage (bug and feedback reports)")]
    Issues(issues::IssuesCommands),

    #[command(subcommand, about = "Report-only licence check-in status")]
    Licence(licence::LicenceCommands),

    #[command(subcommand, about = "Host MCP servers in this binary")]
    Mcp(mcp::McpHostCommands),

    #[command(
        subcommand,
        name = "agent-host",
        about = "Host one agent server in this binary"
    )]
    AgentHost(agent_host::AgentHostCommands),
}

pub async fn execute(cmd: AdminCommands, host: &dyn KernelHost) -> Result<CommandOutput> {
    if host.is_database_scoped()
        && !matches!(
            cmd,
            AdminCommands::Users(_)
                | AdminCommands::Session(_)
                | AdminCommands::Config(config::ConfigCommands::Migrate(_))
        )
    {
        return Err(crate::shared::database_scoped_command_error(host));
    }

    match cmd {
        AdminCommands::Users(cmd) => users::execute(cmd, host).await,
        AdminCommands::AgentHost(cmd) => Box::pin(agent_host::execute(cmd, host)).await,
        AdminCommands::Config(cmd) => config::execute(cmd, host).await,
        AdminCommands::Setup(args) => Box::pin(setup::execute(args, host)).await,
        AdminCommands::Bootstrap(args) => bootstrap::execute(args, host).await,
        AdminCommands::Session(cmd) => session::execute(cmd, host).await,
        AdminCommands::Bridge(cmd) => bridge::execute(cmd, host).await,
        AdminCommands::AccessControl(cmd) => access_control::execute(cmd, host).await,
        AdminCommands::Keys(cmd) => keys::execute(cmd, host).await,
        AdminCommands::Identity(cmd) => identity::execute(cmd, host),
        AdminCommands::Issues(cmd) => Box::pin(issues::execute(cmd, host)).await,
        AdminCommands::Licence(cmd) => licence::execute(cmd, host).await,
        AdminCommands::Mcp(cmd) => mcp::execute(cmd, host).await,
    }
}

impl DescribeCommand for AdminCommands {
    fn descriptor(&self) -> CommandDescriptor {
        match self {
            Self::Session(cmd) => cmd.descriptor(),
            Self::Config(cmd) => cmd.descriptor(),
            Self::AgentHost(_) | Self::Mcp(_) => {
                CommandDescriptor::CHILD_HOST
            },
            Self::Bridge(bridge::BridgeCommands::SigningKeyFingerprint(_)) => {
                CommandDescriptor::PROFILE_AND_SECRETS
            },
            Self::Setup(_) => CommandDescriptor::NONE,
            Self::Licence(_) => CommandDescriptor::FULL.with_read_only(),
            _ => CommandDescriptor::FULL,
        }
    }
}

impl AdminCommands {
    pub const fn data_impact(&self) -> DataImpact {
        match self {
            Self::Users(cmd) => cmd.data_impact(),
            Self::Issues(cmd) => cmd.data_impact(),
            Self::Setup(args) if args.reset_role_password => DataImpact::Irreversible,
            Self::Bridge(bridge::BridgeCommands::RotateSigningKey(_))
            | Self::Keys(
                keys::KeysCommands::Stage(_)
                | keys::KeysCommands::Promote(_)
                | keys::KeysCommands::Rotate(_),
            ) => DataImpact::Irreversible,
            Self::Bootstrap(_)
            | Self::Config(config::ConfigCommands::Secret(
                config::secret::SecretCommands::MigrateGitCredentials(_),
            )) => DataImpact::Destructive,
            Self::Bridge(
                bridge::BridgeCommands::EnrollCert(_)
                | bridge::BridgeCommands::IssueCode(_)
                | bridge::BridgeCommands::List(_)
                | bridge::BridgeCommands::SigningKeyFingerprint(_),
            )
            | Self::AccessControl(
                access_control::AccessControlCommands::ExportYaml(_)
                | access_control::AccessControlCommands::Lint(_),
            )
            | Self::Keys(
                keys::KeysCommands::Generate(_) | keys::KeysCommands::IssuePluginToken(_),
            )
            | Self::Config(_)
            | Self::Setup(_)
            | Self::Session(_)
            | Self::Identity(_)
            | Self::Licence(_)
            | Self::Mcp(_)
            | Self::AgentHost(_) => DataImpact::Preserving,
        }
    }
}
