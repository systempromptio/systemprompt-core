pub mod analytics;
pub mod delete;
pub mod delete_source;
pub mod files;
pub mod ingest;
pub mod link;
pub mod list;
pub mod popular;
pub mod publish;
pub mod search;
pub mod show;
pub mod status;
pub mod types;
pub mod verify;

use crate::cli_settings::{get_global_config, CliConfig};
use crate::shared::render_result;
use anyhow::{bail, Context, Result};
use clap::Subcommand;
use systemprompt_runtime::DatabaseContext;

#[derive(Debug, Subcommand)]
pub enum ContentCommands {
    #[command(about = "List content with pagination")]
    List(list::ListArgs),

    #[command(about = "Show content details")]
    Show(show::ShowArgs),

    #[command(about = "Search content")]
    Search(search::SearchArgs),

    #[command(about = "Ingest markdown files from directory")]
    Ingest(ingest::IngestArgs),

    #[command(about = "Delete content by ID")]
    Delete(delete::DeleteArgs),

    #[command(name = "delete-source", about = "Delete all content from a source")]
    DeleteSource(delete_source::DeleteSourceArgs),

    #[command(about = "Get popular content")]
    Popular(popular::PopularArgs),

    #[command(about = "Verify content is published and accessible")]
    Verify(verify::VerifyArgs),

    #[command(about = "Show content health status for a source")]
    Status(status::StatusArgs),

    #[command(subcommand, about = "Link generation and management")]
    Link(link::LinkCommands),

    #[command(subcommand, about = "Content analytics")]
    Analytics(analytics::AnalyticsCommands),

    #[command(subcommand, about = "Content-file operations (link, unlink, featured)")]
    Files(files::ContentFilesCommands),

    #[command(about = "Publish static content (ingest, prerender, sitemap)")]
    Publish(publish::PublishArgs),

    #[command(about = "Alias for publish", hide = true)]
    Generate(publish::PublishArgs),
}

pub async fn execute(command: ContentCommands) -> Result<()> {
    let config = get_global_config();
    execute_with_config(command, &config).await
}

pub async fn execute_with_config(command: ContentCommands, config: &CliConfig) -> Result<()> {
    match command {
        ContentCommands::List(args) => {
            let result = list::execute(args, config)
                .await
                .context("Failed to list content")?;
            render_result(&result);
        },
        ContentCommands::Show(args) => {
            let result = show::execute(args, config)
                .await
                .context("Failed to show content")?;
            render_result(&result);
        },
        ContentCommands::Search(args) => {
            let result = search::execute(args, config)
                .await
                .context("Failed to search content")?;
            render_result(&result);
        },
        ContentCommands::Ingest(args) => {
            let result = ingest::execute(args, config)
                .await
                .context("Failed to ingest content")?;
            match result {
                ingest::IngestResult::Single(r) => render_result(&r),
                ingest::IngestResult::All(r) => render_result(&r),
            }
        },
        ContentCommands::Delete(args) => {
            let result = delete::execute(args, config)
                .await
                .context("Failed to delete content")?;
            render_result(&result);
        },
        ContentCommands::DeleteSource(args) => {
            let result = delete_source::execute(args, config)
                .await
                .context("Failed to delete source content")?;
            render_result(&result);
        },
        ContentCommands::Popular(args) => {
            let result = popular::execute(args, config)
                .await
                .context("Failed to get popular content")?;
            render_result(&result);
        },
        ContentCommands::Verify(args) => {
            let result = verify::execute(args, config)
                .await
                .context("Failed to verify content")?;
            render_result(&result);
        },
        ContentCommands::Status(args) => {
            let result = status::execute(args, config)
                .await
                .context("Failed to get content status")?;
            render_result(&result);
        },
        ContentCommands::Link(cmd) => {
            link::execute(cmd, config).await?;
        },
        ContentCommands::Analytics(cmd) => {
            analytics::execute(cmd, config).await?;
        },
        ContentCommands::Files(cmd) => {
            files::execute(cmd, config).await?;
        },
        ContentCommands::Publish(args) | ContentCommands::Generate(args) => {
            let result = publish::execute(args, config)
                .await
                .context("Failed to publish content")?;
            render_result(&result);
        },
    }
    Ok(())
}

pub async fn execute_with_db(
    command: ContentCommands,
    db_ctx: &DatabaseContext,
    config: &CliConfig,
) -> Result<()> {
    match command {
        ContentCommands::List(args) => {
            let result = list::execute_with_pool(args, db_ctx.db_pool(), config)
                .await
                .context("Failed to list content")?;
            render_result(&result);
        },
        ContentCommands::Show(args) => {
            let result = show::execute_with_pool(args, db_ctx.db_pool(), config)
                .await
                .context("Failed to show content")?;
            render_result(&result);
        },
        ContentCommands::Search(args) => {
            let result = search::execute_with_pool(args, db_ctx.db_pool(), config)
                .await
                .context("Failed to search content")?;
            render_result(&result);
        },
        ContentCommands::Popular(args) => {
            let result = popular::execute_with_pool(args, db_ctx.db_pool(), config)
                .await
                .context("Failed to get popular content")?;
            render_result(&result);
        },
        ContentCommands::Status(args) => {
            let result = status::execute_with_pool(args, db_ctx.db_pool(), config)
                .await
                .context("Failed to get content status")?;
            render_result(&result);
        },
        ContentCommands::Analytics(cmd) => {
            analytics::execute_with_pool(cmd, db_ctx.db_pool(), config).await?;
        },
        ContentCommands::Ingest(_)
        | ContentCommands::Delete(_)
        | ContentCommands::DeleteSource(_)
        | ContentCommands::Verify(_)
        | ContentCommands::Link(_)
        | ContentCommands::Files(_)
        | ContentCommands::Publish(_)
        | ContentCommands::Generate(_) => {
            bail!("This content command requires full profile context")
        },
    }
    Ok(())
}
