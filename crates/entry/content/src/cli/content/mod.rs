//! `core content` command group: manage and inspect published content.
//!
//! Dispatches the [`ContentCommands`] subcommands — list/show/search,
//! edit/delete, ingest verification and status, plus the `link`, `analytics`,
//! and `files` sub-groups. `content-http` contributes the group under the
//! kernel's `core` group. It needs the full profile context, accepts
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
use systemprompt_extension::cli::{
    CliCommandGroup, CommandDescriptor, CommandError, CommandHost, CommandOutput, DataImpact,
    DescribeCommand,
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

fn ensure_full_profile(command: &ContentCommands, host: &dyn CommandHost) -> Result<()> {
    if host.is_database_scoped()
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

pub async fn execute(command: ContentCommands, host: &dyn CommandHost) -> Result<CommandOutput> {
    ensure_full_profile(&command, host)?;

    match command {
        ContentCommands::List(args) => list::execute(args, host)
            .await
            .context("Failed to list content"),
        ContentCommands::Show(args) => show::execute(args, host)
            .await
            .context("Failed to show content"),
        ContentCommands::Search(args) => search::execute(args, host)
            .await
            .context("Failed to search content"),
        ContentCommands::Edit(args) => edit::execute(args, host)
            .await
            .context("Failed to edit content"),
        ContentCommands::Delete(args) => delete::execute(args, host)
            .await
            .context("Failed to delete content"),
        ContentCommands::DeleteSource(args) => delete_source::execute(args, host)
            .await
            .context("Failed to delete source content"),
        ContentCommands::Verify(args) => verify::execute(args, host)
            .await
            .context("Failed to verify content"),
        ContentCommands::Status(args) => status::execute(args, host)
            .await
            .context("Failed to get content status"),
        ContentCommands::Link(cmd) => link::execute(cmd, host).await,
        ContentCommands::Analytics(cmd) => analytics::execute(cmd, host).await,
        ContentCommands::Files(cmd) => files::execute(cmd, host).await,
    }
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
    const NAME: &'static str = "content";
    const ABOUT: &'static str = "Content management and analytics";


    async fn run(self, host: &dyn CommandHost) -> Result<CommandOutput, CommandError> {
        execute(self, host).await.map_err(CommandError::new)
    }
}
