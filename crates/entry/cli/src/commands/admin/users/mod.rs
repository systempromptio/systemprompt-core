//! User administration command tree.
//!
//! [`UsersCommands`] groups the user CRUD, search, export, stats, merge, and
//! the `bulk`, `role`, `session`, `ban`, and `webauthn` subcommand trees. On a
//! `--database-url` invocation only the read-only commands are served; write
//! operations require a full profile context.

mod ban;
mod bulk;
mod count;
mod create;
pub(crate) mod delete;
mod export;
mod list;
mod merge;
mod role;
mod search;
mod session;
mod show;
mod stats;
mod types;
mod update;
mod webauthn;

use crate::context::CommandContext;
use crate::shared::render_result;
use anyhow::{Result, bail};
use clap::Subcommand;

pub use types::*;

#[derive(Debug, Subcommand)]
pub enum UsersCommands {
    #[command(about = "List users with pagination and filtering")]
    List(list::ListArgs),

    #[command(about = "Show detailed user information")]
    Show(show::ShowArgs),

    #[command(about = "Search users by name, email, or full name")]
    Search(search::SearchArgs),

    #[command(about = "Create a new user")]
    Create(create::CreateArgs),

    #[command(about = "Update user fields")]
    Update(update::UpdateArgs),

    #[command(about = "Delete a user")]
    Delete(delete::DeleteArgs),

    #[command(about = "Get total user count")]
    Count(count::CountArgs),

    #[command(about = "Export users to JSON")]
    Export(export::ExportArgs),

    #[command(about = "Show user statistics dashboard")]
    Stats,

    #[command(about = "Merge source user into target user")]
    Merge(merge::MergeArgs),

    #[command(subcommand, about = "Bulk operations on users")]
    Bulk(bulk::BulkCommands),

    #[command(subcommand, about = "Role management commands")]
    Role(role::RoleCommands),

    #[command(subcommand, about = "Session management commands")]
    Session(session::SessionCommands),

    #[command(subcommand, about = "IP ban management commands")]
    Ban(ban::BanCommands),

    #[command(subcommand, about = "WebAuthn credential management commands")]
    Webauthn(webauthn::WebauthnCommands),
}

pub async fn execute(cmd: UsersCommands, ctx: &CommandContext) -> Result<()> {
    if ctx.is_database_scoped()
        && matches!(
            cmd,
            UsersCommands::Create(_)
                | UsersCommands::Update(_)
                | UsersCommands::Delete(_)
                | UsersCommands::Merge(_)
                | UsersCommands::Bulk(_)
                | UsersCommands::Webauthn(_)
        )
    {
        bail!("Write operations require full profile context");
    }

    match cmd {
        UsersCommands::List(args) => {
            let result = list::execute(args, ctx).await?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
        UsersCommands::Show(args) => {
            let result = show::execute(args, ctx).await?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
        UsersCommands::Search(args) => {
            let result = search::execute(args, ctx).await?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
        UsersCommands::Create(args) => {
            let result = create::execute(args, ctx).await?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
        UsersCommands::Update(args) => {
            let result = update::execute(args, ctx).await?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
        UsersCommands::Delete(args) => {
            let result = delete::execute(args, ctx).await?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
        UsersCommands::Count(args) => {
            let result = count::execute(args, ctx).await?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
        UsersCommands::Export(args) => {
            let result = export::execute(args, ctx).await?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
        UsersCommands::Stats => {
            let result = stats::execute(ctx).await?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
        UsersCommands::Merge(args) => {
            let result = merge::execute(args, ctx).await?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
        UsersCommands::Bulk(cmd) => bulk::execute(cmd, ctx).await,
        UsersCommands::Role(cmd) => role::execute(cmd, ctx).await,
        UsersCommands::Session(cmd) => session::execute(cmd, ctx).await,
        UsersCommands::Ban(cmd) => ban::execute(cmd, ctx).await,
        UsersCommands::Webauthn(cmd) => webauthn::execute(cmd, ctx).await,
    }
}
