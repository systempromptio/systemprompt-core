pub mod artifacts;
pub mod content;
pub mod contexts;
pub mod files;
pub mod playbooks;
pub mod skills;

use anyhow::Result;
use clap::Subcommand;
use systemprompt_runtime::DatabaseContext;

use crate::CliConfig;

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

    #[command(subcommand, about = "Playbook management and database sync")]
    Playbooks(playbooks::PlaybooksCommands),

    #[command(subcommand, about = "Skill management and database sync")]
    Skills(skills::SkillsCommands),
}

pub async fn execute(cmd: CoreCommands, config: &CliConfig) -> Result<()> {
    match cmd {
        CoreCommands::Artifacts(cmd) => artifacts::execute(cmd, config).await,
        CoreCommands::Content(cmd) => content::execute(cmd).await,
        CoreCommands::Files(cmd) => files::execute(cmd, config).await,
        CoreCommands::Contexts(cmd) => contexts::execute(cmd, config).await,
        CoreCommands::Playbooks(cmd) => playbooks::execute(cmd, config).await,
        CoreCommands::Skills(cmd) => skills::execute(cmd).await,
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
        CoreCommands::Artifacts(_)
        | CoreCommands::Contexts(_)
        | CoreCommands::Playbooks(_)
        | CoreCommands::Skills(_) => {
            anyhow::bail!("This command requires full profile context")
        },
    }
}
