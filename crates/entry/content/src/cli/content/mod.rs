//! `core content` command group: manage and inspect published content.
//!
//! Dispatches the [`ContentCommands`] subcommands — list/show/search,
//! edit/delete, ingest verification and status, plus the `link`, `analytics`,
//! and `files` sub-groups. The group registers under the kernel's `core` group
//! through `register_cli_command!`. It needs the full profile context, accepts
//! a direct `--database-url`, and on such an invocation serves only the
//! read-only commands; the mutating commands require a full profile context.
//! Deleting content, a source's content or a link is destructive; every other
//! command preserves the data behind the profile.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod analytics;
pub mod delete;
pub mod delete_source;
pub mod edit;
pub mod edit_apply;
pub mod files;
pub mod link;
pub mod list;
pub mod search;
pub mod show;
pub mod status;
pub mod types;
pub mod verify;

use anyhow::{Context, Result, bail};
use clap::Subcommand;
use systemprompt_cli::kernel::{
    CliCommandGroup, CommandContext, CommandDescriptor, DataImpact, DbUrlRouting, DescribeCommand,
    render_result,
};

#[derive(Debug, Subcommand)]
pub enum ContentCommands {
    #[command(about = "List content with pagination")]
    List(list::ListArgs),

    #[command(about = "Show content details")]
    Show(show::ShowArgs),

    #[command(about = "Search content")]
    Search(search::SearchArgs),

    #[command(about = "Edit content fields")]
    Edit(edit::EditArgs),

    #[command(about = "Delete content by ID")]
    Delete(delete::DeleteArgs),

    #[command(name = "delete-source", about = "Delete all content from a source")]
    DeleteSource(delete_source::DeleteSourceArgs),

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
}

fn ensure_full_profile(command: &ContentCommands, ctx: &CommandContext) -> Result<()> {
    if ctx.is_database_scoped()
        && matches!(
            command,
            ContentCommands::Edit(_)
                | ContentCommands::Delete(_)
                | ContentCommands::DeleteSource(_)
                | ContentCommands::Verify(_)
                | ContentCommands::Link(_)
                | ContentCommands::Files(_)
        )
    {
        bail!("This content command requires full profile context");
    }
    Ok(())
}

pub async fn execute(command: ContentCommands, ctx: &CommandContext) -> Result<()> {
    ensure_full_profile(&command, ctx)?;

    match command {
        ContentCommands::List(args) => {
            let result = list::execute(args, ctx)
                .await
                .context("Failed to list content")?;
            render_result(&result, &ctx.cli);
        },
        ContentCommands::Show(args) => {
            let result = show::execute(args, ctx)
                .await
                .context("Failed to show content")?;
            render_result(&result, &ctx.cli);
        },
        ContentCommands::Search(args) => {
            let result = search::execute(args, ctx)
                .await
                .context("Failed to search content")?;
            render_result(&result, &ctx.cli);
        },
        ContentCommands::Edit(args) => {
            let result = edit::execute(args, ctx)
                .await
                .context("Failed to edit content")?;
            render_result(&result, &ctx.cli);
        },
        ContentCommands::Delete(args) => {
            let result = delete::execute(args, ctx)
                .await
                .context("Failed to delete content")?;
            render_result(&result, &ctx.cli);
        },
        ContentCommands::DeleteSource(args) => {
            let result = delete_source::execute(args, ctx)
                .await
                .context("Failed to delete source content")?;
            render_result(&result, &ctx.cli);
        },
        ContentCommands::Verify(args) => {
            let result = verify::execute(args, ctx)
                .await
                .context("Failed to verify content")?;
            render_result(&result, &ctx.cli);
        },
        ContentCommands::Status(args) => {
            let result = status::execute(args, ctx)
                .await
                .context("Failed to get content status")?;
            render_result(&result, &ctx.cli);
        },
        ContentCommands::Link(cmd) => {
            link::execute(cmd, ctx).await?;
        },
        ContentCommands::Analytics(cmd) => {
            analytics::execute(cmd, ctx).await?;
        },
        ContentCommands::Files(cmd) => {
            files::execute(cmd, ctx.prompter(), ctx).await?;
        },
    }
    Ok(())
}

impl ContentCommands {
    pub const fn data_impact(&self) -> DataImpact {
        match self {
            Self::Delete(_) | Self::DeleteSource(_) | Self::Link(link::LinkCommands::Delete(_)) => {
                DataImpact::Destructive
            },
            Self::List(_)
            | Self::Show(_)
            | Self::Search(_)
            | Self::Edit(_)
            | Self::Verify(_)
            | Self::Status(_)
            | Self::Link(
                link::LinkCommands::Generate(_)
                | link::LinkCommands::Show(_)
                | link::LinkCommands::List(_)
                | link::LinkCommands::Performance(_),
            )
            | Self::Analytics(_)
            | Self::Files(_) => DataImpact::Preserving,
        }
    }
}

impl DescribeCommand for ContentCommands {
    fn descriptor(&self) -> CommandDescriptor {
        CommandDescriptor::FULL.with_data_impact(self.data_impact())
    }
}

impl CliCommandGroup for ContentCommands {
    const PARENT: &'static [&'static str] = &["core"];
    const NAME: &'static str = "content";
    const ABOUT: &'static str = "Content management and analytics";

    fn db_url_routing(&self) -> DbUrlRouting {
        DbUrlRouting::Direct
    }

    fn run(self, ctx: &CommandContext) -> impl Future<Output = Result<()>> + Send {
        execute(self, ctx)
    }
}

systemprompt_cli::register_cli_command!(ContentCommands);
