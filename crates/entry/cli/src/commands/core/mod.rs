//! `core` command group: the platform's primary domain commands.
//!
//! Dispatches the [`CoreCommands`] subgroups — artifacts, files, contexts,
//! skills, plugins, marketplace, hooks and services bundles. On a
//! `--database-url` invocation only the files subgroup is served; the rest
//! require a full profile context. Capabilities contribute further
//! subgroups under `core` (such as `core content`) with
//! `CommandDecl::group_under`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod artifacts;
pub mod contexts;
pub mod files;
pub mod hooks;
pub mod marketplace;
pub mod plugins;
pub mod services;
pub mod skills;

use anyhow::Result;
use crate::kernel::KernelHost;
use clap::Subcommand;

use systemprompt_extension::cli::{
    CommandDescriptor, CommandOutput, DataImpact, DescribeCommand,
};

#[derive(Debug, Subcommand)]
pub enum CoreCommands {
    #[command(subcommand, about = "Artifact inspection and debugging")]
    Artifacts(artifacts::ArtifactsCommands),

    #[command(subcommand, about = "File management and uploads")]
    Files(files::FilesCommands),

    #[command(subcommand, about = "Context management")]
    Contexts(contexts::ContextsCommands),

    #[command(subcommand, about = "Skill management and database sync")]
    Skills(skills::SkillsCommands),

    #[command(subcommand, about = "Plugin management and marketplace generation")]
    Plugins(plugins::PluginsCommands),

    #[command(subcommand, about = "Marketplace manifest diagnostics")]
    Marketplace(marketplace::MarketplaceCommands),

    #[command(subcommand, about = "Hook validation and inspection")]
    Hooks(hooks::HooksCommands),

    #[command(
        subcommand,
        about = "Services bundles: validate, pack, sign, publish, refresh, inspect"
    )]
    Services(services::ServicesCommands),
}

pub async fn execute(cmd: CoreCommands, host: &dyn KernelHost) -> Result<CommandOutput> {
    if host.is_database_scoped() && !matches!(cmd, CoreCommands::Files(_)) {
        return Err(crate::shared::database_scoped_command_error(host));
    }

    match cmd {
        CoreCommands::Artifacts(cmd) => Box::pin(artifacts::execute(cmd, host)).await,
        CoreCommands::Files(cmd) => files::execute(cmd, host).await,
        CoreCommands::Contexts(cmd) => Box::pin(contexts::execute(cmd, host)).await,
        CoreCommands::Skills(cmd) => skills::execute(cmd, host).await,
        CoreCommands::Plugins(cmd) => plugins::execute(cmd, host),
        CoreCommands::Marketplace(cmd) => marketplace::execute(cmd, host).await,
        CoreCommands::Hooks(cmd) => hooks::execute(cmd, host),
        CoreCommands::Services(cmd) => Box::pin(services::execute(cmd, host)).await,
    }
}

impl DescribeCommand for CoreCommands {
    fn descriptor(&self) -> CommandDescriptor {
        match self {
            Self::Marketplace(marketplace::MarketplaceCommands::Import(_))
            | Self::Services(
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
            Self::Hooks(_)
            | Self::Plugins(_)
            | Self::Skills(skills::SkillsCommands::List(_) | skills::SkillsCommands::Show(_)) => {
                CommandDescriptor::PROFILE_SECRETS_AND_PATHS
            },
            _ => CommandDescriptor::FULL,
        }
    }
}

impl CoreCommands {
    pub const fn data_impact(&self) -> DataImpact {
        match self {
            Self::Files(files::FilesCommands::Delete(_))
            | Self::Contexts(contexts::ContextsCommands::Delete(_))
            | Self::Services(
                services::ServicesCommands::Refresh(_) | services::ServicesCommands::Publish(_),
            ) => DataImpact::Destructive,
            Self::Files(
                files::FilesCommands::List(_)
                | files::FilesCommands::Show(_)
                | files::FilesCommands::Upload(_)
                | files::FilesCommands::Validate(_)
                | files::FilesCommands::Config(_)
                | files::FilesCommands::Search(_)
                | files::FilesCommands::Stats(_)
                | files::FilesCommands::Ai(_),
            )
            | Self::Contexts(
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
            | Self::Skills(_)
            | Self::Plugins(_)
            | Self::Marketplace(_)
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
