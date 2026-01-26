mod list;
mod show;
mod types;

use crate::cli_settings::CliConfig;
use crate::shared::render_result;
use anyhow::Result;
use clap::Subcommand;
use systemprompt_database::DbPool;

pub use types::*;

#[derive(Debug, Subcommand)]
pub enum ArtifactsCommands {
    #[command(about = "List artifacts")]
    List(list::ListArgs),

    #[command(about = "Show artifact details and content")]
    Show(show::ShowArgs),
}

pub async fn execute(cmd: ArtifactsCommands, config: &CliConfig) -> Result<()> {
    match cmd {
        ArtifactsCommands::List(args) => {
            let result = list::execute(args, config).await?;
            render_result(&result);
        },
        ArtifactsCommands::Show(args) => {
            let result = show::execute(args, config).await?;
            render_result(&result);
        },
    }
    Ok(())
}

pub async fn execute_with_db(
    cmd: ArtifactsCommands,
    db_pool: &DbPool,
    user_id: &systemprompt_identifiers::UserId,
    config: &CliConfig,
) -> Result<()> {
    match cmd {
        ArtifactsCommands::List(args) => {
            let result = list::execute_with_pool(args, user_id, db_pool, config).await?;
            render_result(&result);
        },
        ArtifactsCommands::Show(args) => {
            let result = show::execute_with_pool(args, db_pool, config).await?;
            render_result(&result);
        },
    }
    Ok(())
}
