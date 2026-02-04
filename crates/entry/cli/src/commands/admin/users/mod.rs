mod ban;
mod bulk;
mod count;
mod create;
mod delete;
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

use crate::cli_settings::CliConfig;
use crate::shared::render_result;
use anyhow::{bail, Result};
use clap::Subcommand;
use systemprompt_runtime::DatabaseContext;

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

pub async fn execute(cmd: UsersCommands, config: &CliConfig) -> Result<()> {
    match cmd {
        UsersCommands::List(args) => {
            let result = list::execute(args, config).await?;
            render_result(&result);
            Ok(())
        },
        UsersCommands::Show(args) => {
            let result = show::execute(args, config).await?;
            render_result(&result);
            Ok(())
        },
        UsersCommands::Search(args) => {
            let result = search::execute(args, config).await?;
            render_result(&result);
            Ok(())
        },
        UsersCommands::Create(args) => {
            let result = create::execute(args, config).await?;
            render_result(&result);
            Ok(())
        },
        UsersCommands::Update(args) => {
            let result = update::execute(args, config).await?;
            render_result(&result);
            Ok(())
        },
        UsersCommands::Delete(args) => {
            let result = delete::execute(args, config).await?;
            render_result(&result);
            Ok(())
        },
        UsersCommands::Count(args) => {
            let result = count::execute(args, config).await?;
            render_result(&result);
            Ok(())
        },
        UsersCommands::Export(args) => {
            let result = export::execute(args, config).await?;
            render_result(&result);
            Ok(())
        },
        UsersCommands::Stats => {
            let result = stats::execute(config).await?;
            render_result(&result);
            Ok(())
        },
        UsersCommands::Merge(args) => {
            let result = merge::execute(args, config).await?;
            render_result(&result);
            Ok(())
        },
        UsersCommands::Bulk(cmd) => bulk::execute(cmd, config).await,
        UsersCommands::Role(cmd) => role::execute(cmd, config).await,
        UsersCommands::Session(cmd) => session::execute(cmd, config).await,
        UsersCommands::Ban(cmd) => ban::execute(cmd, config).await,
        UsersCommands::Webauthn(cmd) => webauthn::execute(cmd, config).await,
    }
}

pub async fn execute_with_db(
    cmd: UsersCommands,
    db_ctx: &DatabaseContext,
    config: &CliConfig,
) -> Result<()> {
    match cmd {
        UsersCommands::List(args) => {
            let result = list::execute_with_pool(args, db_ctx.db_pool(), config).await?;
            render_result(&result);
            Ok(())
        },
        UsersCommands::Show(args) => {
            let result = show::execute_with_pool(args, db_ctx.db_pool(), config).await?;
            render_result(&result);
            Ok(())
        },
        UsersCommands::Search(args) => {
            let result = search::execute_with_pool(args, db_ctx.db_pool(), config).await?;
            render_result(&result);
            Ok(())
        },
        UsersCommands::Count(args) => {
            let result = count::execute_with_pool(args, db_ctx.db_pool(), config).await?;
            render_result(&result);
            Ok(())
        },
        UsersCommands::Export(args) => {
            let result = export::execute_with_pool(args, db_ctx.db_pool(), config).await?;
            render_result(&result);
            Ok(())
        },
        UsersCommands::Stats => {
            let result = stats::execute_with_pool(db_ctx.db_pool(), config).await?;
            render_result(&result);
            Ok(())
        },
        UsersCommands::Session(cmd) => {
            session::execute_with_pool(cmd, db_ctx.db_pool(), config).await
        },
        UsersCommands::Ban(cmd) => ban::execute_with_pool(cmd, db_ctx.db_pool(), config).await,
        UsersCommands::Role(cmd) => role::execute_with_pool(cmd, db_ctx.db_pool(), config),
        UsersCommands::Create(_)
        | UsersCommands::Update(_)
        | UsersCommands::Delete(_)
        | UsersCommands::Merge(_)
        | UsersCommands::Bulk(_)
        | UsersCommands::Webauthn(_) => {
            bail!("Write operations require full profile context")
        },
    }
}
