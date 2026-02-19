pub mod agents;
pub mod artifacts;
pub mod content;
pub mod contexts;
pub mod files;
pub mod hooks;
pub mod plugins;
pub mod skills;

use anyhow::Result;
use clap::Subcommand;
use systemprompt_runtime::DatabaseContext;

use crate::CliConfig;

#[derive(Debug, Subcommand)]
pub enum CoreCommands {
    #[command(subcommand, about = "Agent entity management and database sync")]
    Agents(agents::AgentsCommands),

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

pub async fn execute(cmd: CoreCommands, config: &CliConfig) -> Result<()> {
    match cmd {
        CoreCommands::Agents(cmd) => agents::execute(cmd).await,
        CoreCommands::Artifacts(cmd) => artifacts::execute(cmd, config).await,
        CoreCommands::Content(cmd) => content::execute(cmd).await,
        CoreCommands::Files(cmd) => files::execute(cmd, config).await,
        CoreCommands::Contexts(cmd) => contexts::execute(cmd, config).await,
        CoreCommands::Skills(cmd) => skills::execute(cmd).await,
        CoreCommands::Plugins(cmd) => plugins::execute(cmd),
        CoreCommands::Hooks(cmd) => hooks::execute(cmd),
    }
}

pub async fn execute_with_db(
    cmd: CoreCommands,
    db_ctx: &DatabaseContext,
    config: &CliConfig,
) -> Result<()> {
    match cmd {
        CoreCommands::Content(cmd) => content::execute_with_db(cmd, db_ctx, config).await,
        CoreCommands::Files(cmd) => files::execute_with_db(cmd, db_ctx, config).await,
        CoreCommands::Agents(_)
        | CoreCommands::Artifacts(_)
        | CoreCommands::Contexts(_)
        | CoreCommands::Skills(_)
        | CoreCommands::Plugins(_)
        | CoreCommands::Hooks(_) => {
            anyhow::bail!("This command requires full profile context")
        },
    }
}
