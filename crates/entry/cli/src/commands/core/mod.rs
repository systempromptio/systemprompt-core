//! `core` command group: the platform's primary domain commands.
//!
//! Dispatches the [`CoreCommands`] subgroups — artifacts, content, files,
//! contexts, skills, plugins, marketplace, and hooks. On a `--database-url`
//! invocation only the content and files subgroups are served; the rest require
//! a full profile context.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod artifacts;
pub mod content;
pub mod contexts;
pub mod files;
pub mod hooks;
pub mod marketplace;
pub mod plugins;
pub mod services;
pub mod skills;

use anyhow::Result;
use clap::Subcommand;

use crate::context::CommandContext;
use crate::descriptor::DataImpact;

#[derive(Debug, Subcommand)]
pub enum CoreCommands {
    #[command(subcommand, about = "Artifact inspection and debugging")]
    Artifacts(artifacts::ArtifactsCommands),

    #[command(subcommand, about = "Content management and analytics")]
    Content(content::ContentCommands),

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

pub async fn execute(cmd: CoreCommands, ctx: &CommandContext) -> Result<()> {
    if ctx.is_database_scoped() && !matches!(cmd, CoreCommands::Content(_) | CoreCommands::Files(_))
    {
        return Err(crate::shared::database_scoped_command_error());
    }

    match cmd {
        CoreCommands::Artifacts(cmd) => artifacts::execute(cmd, ctx).await,
        CoreCommands::Content(cmd) => content::execute(cmd, ctx).await,
        CoreCommands::Files(cmd) => files::execute(cmd, ctx).await,
        CoreCommands::Contexts(cmd) => contexts::execute(cmd, ctx).await,
        CoreCommands::Skills(cmd) => skills::execute(cmd, ctx).await,
        CoreCommands::Plugins(cmd) => plugins::execute(cmd, ctx),
        CoreCommands::Marketplace(cmd) => marketplace::execute(cmd, ctx).await,
        CoreCommands::Hooks(cmd) => hooks::execute(cmd, ctx),
        CoreCommands::Services(cmd) => Box::pin(services::execute(cmd, ctx)).await,
    }
}

impl CoreCommands {
    pub const fn data_impact(&self) -> DataImpact {
        use content::ContentCommands;
        use content::link::LinkCommands;

        match self {
            Self::Content(
                ContentCommands::Delete(_)
                | ContentCommands::DeleteSource(_)
                | ContentCommands::Link(LinkCommands::Delete(_)),
            )
            | Self::Files(files::FilesCommands::Delete(_))
            | Self::Contexts(contexts::ContextsCommands::Delete(_)) => DataImpact::Destructive,
            Self::Content(
                ContentCommands::List(_)
                | ContentCommands::Show(_)
                | ContentCommands::Search(_)
                | ContentCommands::Edit(_)
                | ContentCommands::Popular(_)
                | ContentCommands::Verify(_)
                | ContentCommands::Status(_)
                | ContentCommands::Link(
                    LinkCommands::Generate(_)
                    | LinkCommands::Show(_)
                    | LinkCommands::List(_)
                    | LinkCommands::Performance(_),
                )
                | ContentCommands::Analytics(_)
                | ContentCommands::Files(_),
            )
            | Self::Files(
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
            | Self::Services(_) => DataImpact::Preserving,
        }
    }
}
