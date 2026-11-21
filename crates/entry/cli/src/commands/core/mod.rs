//! `core` command group: the platform's primary domain commands.
//!
//! Dispatches the [`CoreCommands`] subgroups — artifacts, contexts, hooks
//! and services bundles. Each requires a full profile context; none is
//! served on a `--database-url` invocation. Capabilities contribute further
//! subgroups under `core` (such as `core content`, `core files`,
//! `core marketplace`, `core plugins` and `core skills`) with
//! `CommandDecl::group_under`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod artifacts;
pub mod contexts;
pub mod hooks;
pub mod services;

use crate::kernel::KernelHost;
use anyhow::Result;
use clap::Subcommand;

use systemprompt_extension::cli::{CommandDescriptor, CommandOutput, DataImpact, DescribeCommand};

#[derive(Debug, Subcommand)]
pub enum CoreCommands {
    #[command(subcommand, about = "Artifact inspection and debugging")]
    Artifacts(artifacts::ArtifactsCommands),

    #[command(subcommand, about = "Context management")]
    Contexts(contexts::ContextsCommands),

    #[command(subcommand, about = "Hook validation and inspection")]
    Hooks(hooks::HooksCommands),

    #[command(
        subcommand,
        about = "Services bundles: validate, pack, sign, publish, refresh, inspect"
    )]
    Services(services::ServicesCommands),
}

pub async fn execute(cmd: CoreCommands, host: &dyn KernelHost) -> Result<CommandOutput> {
    if host.is_database_scoped() {
        return Err(crate::shared::database_scoped_command_error(host));
    }

    match cmd {
        CoreCommands::Artifacts(cmd) => Box::pin(artifacts::execute(cmd, host)).await,
        CoreCommands::Contexts(cmd) => Box::pin(contexts::execute(cmd, host)).await,
        CoreCommands::Hooks(cmd) => hooks::execute(cmd, host),
        CoreCommands::Services(cmd) => Box::pin(services::execute(cmd, host)).await,
    }
}

impl DescribeCommand for CoreCommands {
    fn descriptor(&self) -> CommandDescriptor {
        match self {
            Self::Services(
                services::ServicesCommands::Validate(_)
                | services::ServicesCommands::Bundle(_)
                | services::ServicesCommands::Keygen(_),
            ) => CommandDescriptor::NONE,
            Self::Services(services::ServicesCommands::Publish(args)) => {
                if args.auth_secret.is_some() {
                    CommandDescriptor::PROFILE_AND_SECRETS
                } else {
                    CommandDescriptor::NONE
                }
            },
            Self::Services(services::ServicesCommands::Inspect(args)) => {
                if args.active {
                    CommandDescriptor::PROFILE_SECRETS_AND_PATHS
                } else {
                    CommandDescriptor::NONE
                }
            },
            Self::Hooks(_) => CommandDescriptor::PROFILE_SECRETS_AND_PATHS,
            _ => CommandDescriptor::FULL,
        }
    }
}

impl CoreCommands {
    pub const fn data_impact(&self) -> DataImpact {
        match self {
            Self::Contexts(contexts::ContextsCommands::Delete(_))
            | Self::Services(
                services::ServicesCommands::Refresh(_) | services::ServicesCommands::Publish(_),
            ) => DataImpact::Destructive,
            Self::Contexts(
                contexts::ContextsCommands::List(_)
                | contexts::ContextsCommands::Show(_)
                | contexts::ContextsCommands::Create(_)
                | contexts::ContextsCommands::Edit(_)
                | contexts::ContextsCommands::Use(_)
                | contexts::ContextsCommands::New(_),
            )
            | Self::Artifacts(
                artifacts::ArtifactsCommands::List(_) | artifacts::ArtifactsCommands::Show(_),
            )
            | Self::Hooks(_)
            | Self::Services(
                services::ServicesCommands::Validate(_)
                | services::ServicesCommands::Bundle(_)
                | services::ServicesCommands::Keygen(_)
                | services::ServicesCommands::Inspect(_),
            ) => DataImpact::Preserving,
        }
    }
}
