//! `core` command group: the platform's primary domain commands.
//!
//! Dispatches the [`CoreCommands`] subgroups — artifacts, content, files,
//! contexts, skills, plugins, and hooks. On a `--database-url` invocation only
//! the content and files subgroups are served; the rest require a full
//! profile context.

pub mod artifacts;
pub mod content;
pub mod contexts;
pub mod files;
pub mod hooks;
pub mod plugins;
pub mod skills;

use anyhow::Result;
use clap::Subcommand;

use crate::context::CommandContext;

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

    #[command(subcommand, about = "Hook validation and inspection")]
    Hooks(hooks::HooksCommands),
}

pub async fn execute(cmd: CoreCommands, ctx: &CommandContext) -> Result<()> {
    if ctx.is_database_scoped()
        && !matches!(cmd, CoreCommands::Content(_) | CoreCommands::Files(_))
    {
        anyhow::bail!("This command requires full profile context");
    }

    match cmd {
        CoreCommands::Artifacts(cmd) => artifacts::execute(cmd, ctx).await,
        CoreCommands::Content(cmd) => content::execute(cmd, ctx).await,
        CoreCommands::Files(cmd) => files::execute(cmd, ctx).await,
        CoreCommands::Contexts(cmd) => contexts::execute(cmd, ctx).await,
        CoreCommands::Skills(cmd) => skills::execute(cmd, ctx),
        CoreCommands::Plugins(cmd) => plugins::execute(cmd, ctx),
        CoreCommands::Hooks(cmd) => hooks::execute(cmd, ctx),
    }
}
